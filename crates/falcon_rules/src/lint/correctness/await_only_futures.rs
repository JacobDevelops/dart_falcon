//! Await only values assignable to futures; `FutureOr`, dynamic, bottom types
//! and extension types are accepted like the analyzer. A type parameter is
//! assignable only through a bound that is a `Future` subtype, so its bound
//! gets no `FutureOr` or extension-type exemption. Diagnostics underline only `await`.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AwaitOnlyFutures;

impl Rule for AwaitOnlyFutures {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics) }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Await(await_expr) = node else {
            return;
        };
        let expr = await_expr.expr();
        let span = await_expr.span();
        if matches!(expr.unparenthesized(), Expr::NullLit(_)) {
            return;
        }
        let ty = state.infer(expr);
        if awaitable(&ty, state) == Truth::No {
            self.diagnostics.push(self.ctx.diagnostic("Await only futures or other future-like values.", Span::new(span.start, span.start + "await".len())));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn reports_only_the_await_keyword() {
        let source = "Future<void> f() async { await 42; await ('text'); }";
        let diagnostics = inference::analyze(AwaitOnlyFutures::NAME, &AwaitOnlyFutures, Path::new("await_span.dart"),
        source,
        &FalconConfig::default(),);
        assert_eq!(diagnostics.len(), 2);
        for (diagnostic, (start, _)) in diagnostics.iter().zip(source.match_indices("await")) {
            assert_eq!(diagnostic.span.start, start);
            assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "await");
        }
    }
}
