//! Flags a function literal passed to an Iterable's `forEach`.
//!
//! A `for`-in loop reads more clearly than `forEach` with a closure, and unlike
//! the closure it can `await`, `break`, and `continue`. Stream and Map receivers,
//! tear-offs, null-aware chains, and receivers reached through method calls are
//! left alone, matching the SDK lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidFunctionLiteralsInForeachCalls;

impl Rule for AvoidFunctionLiteralsInForeachCalls {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Follow property selectors, not arbitrary descendants: calls in an index or
/// constructor argument do not make the receiver a method chain.
fn has_method_chain(mut receiver: &Expr, state: &SemanticState<'_>) -> bool {
    loop {
        match receiver {
            Expr::Field { object, .. } => receiver = object,
            Expr::Call { callee, .. } => {
                return !state.is_function_expression_invocation(callee)
                    && !state.is_constructor_invocation(callee);
            }
            _ => return false,
        }
    }
}

fn has_null_aware_chain(mut receiver: &Expr, state: &SemanticState<'_>) -> bool {
    loop {
        match receiver {
            Expr::Field {
                object,
                is_null_safe,
                ..
            }
            | Expr::Index {
                object,
                is_null_safe,
                ..
            } => {
                if *is_null_safe {
                    return true;
                }
                receiver = object;
            }
            Expr::Call { callee, .. } => {
                if state.is_function_expression_invocation(callee) {
                    return false;
                }
                receiver = callee;
            }
            _ => return false,
        }
    }
}

fn is_iterable(receiver: &Expr, state: &SemanticState<'_>) -> bool {
    let inferred = state.infer(receiver);
    let mut ty = &inferred;
    while let ResolvedType::TypeParameter { bound, .. }
    | ResolvedType::NonNullTypeParameter { bound, .. } = ty
    {
        ty = bound;
    }
    let erased;
    if matches!(
        ty,
        ResolvedType::Interface {
            extension_type: true,
            ..
        }
    ) {
        let Some(representation) = state.signatures.extension_type_erasure(ty, &state.model) else {
            return false;
        };
        erased = representation;
        ty = &erased;
    }
    state.signatures.is_subtype_of(
        ty,
        &DeclarationIdentity::sdk("dart:core", "Iterable"),
        &state.model,
    ) == Truth::Yes
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Call { callee, args, .. } = node
            && let Expr::Field {
                object,
                field,
                is_null_safe: false,
                ..
            } = callee.as_ref()
            && field.name == "forEach"
            && matches!(args.positional.first(), Some(Expr::FuncExpr { .. }))
            && !state.inside_cascade
            && !state.is_function_expression_invocation(callee)
            && !has_method_chain(object, state)
            && !has_null_aware_chain(object, state)
            && is_iterable(object, state)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-function-literals-in-foreach-calls",
                "Avoid using a function literal in a 'forEach' call; use a for-in loop instead.",
                field.span,
            ));
        }
    }
}
