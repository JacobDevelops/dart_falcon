//! Await only values assignable to futures; `FutureOr`, dynamic, bottom types
//! and extension types are accepted like the analyzer. A type parameter is
//! assignable only through a bound that is a `Future` subtype, so its bound
//! gets no `FutureOr` or extension-type exemption.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AwaitOnlyFutures;

impl Rule for AwaitOnlyFutures {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Await { expr, span } = node else {
            return;
        };
        if matches!(expr.unparenthesized(), Expr::NullLit { .. }) {
            return;
        }
        let ty = state.infer(expr);
        if awaitable(&ty, state) == Truth::No {
            self.diagnostics.push(self.ctx.diagnostic(
                "await-only-futures",
                "Await only futures or other future-like values.",
                *span,
            ));
        }
    }
}

fn awaitable(ty: &ResolvedType, state: &SemanticState<'_>) -> Truth {
    match ty {
        ResolvedType::TypeParameter { bound, .. }
        | ResolvedType::NonNullTypeParameter { bound, .. } => future_subtype(bound, state),
        _ => state.model.is_future_like(ty),
    }
}

/// Whether the type parameter bound `bound` is a subtype of `Future`: neither
/// `FutureOr` nor `dynamic` is.
fn future_subtype(bound: &ResolvedType, state: &SemanticState<'_>) -> Truth {
    match bound {
        ResolvedType::TypeParameter { bound, .. }
        | ResolvedType::NonNullTypeParameter { bound, .. } => future_subtype(bound, state),
        ResolvedType::Dynamic => Truth::No,
        ResolvedType::Interface { identity, .. } if identity.is_sdk("dart:async", "FutureOr") => {
            Truth::No
        }
        ResolvedType::Interface {
            extension_type: true,
            ..
        } => {
            if state
                .signatures
                .instantiated_supertype(bound, "dart:async", "Future", &state.model)
                .is_some()
            {
                Truth::Yes
            } else {
                Truth::Unknown
            }
        }
        _ => state.model.is_future_like(bound),
    }
}
