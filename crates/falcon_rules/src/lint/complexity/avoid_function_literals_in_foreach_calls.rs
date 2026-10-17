//! Flags a function literal passed to an Iterable's `forEach`.
//!
//! A `for`-in loop reads more clearly than `forEach` with a closure, and unlike
//! the closure it can `await`, `break`, and `continue`. Stream and Map receivers,
//! tear-offs, null-aware chains, and receivers reached through method calls are
//! left alone, matching the SDK lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidFunctionLiteralsInForeachCalls;

impl Rule for AvoidFunctionLiteralsInForeachCalls {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Follow property selectors, not arbitrary descendants: calls in an index or
/// constructor argument do not make the receiver a method chain.
fn has_method_chain<'a>(mut receiver: Expr<'a>, state: &SemanticState<'a>) -> bool {
    loop {
        match receiver {
            Expr::Field(field) => receiver = field.object(),
            Expr::Call(call) => {
                return !state.is_function_expression_invocation(call.callee())
                    && !state.is_constructor_invocation(call.callee());
            }
            _ => return false,
        }
    }
}

fn has_null_aware_chain<'a>(mut receiver: Expr<'a>, state: &SemanticState<'a>) -> bool {
    loop {
        match receiver {
            Expr::Field(field) => {
                if field.is_null_safe() {
                    return true;
                }
                receiver = field.object();
            }
            Expr::Index(index) => {
                if index.is_null_safe() {
                    return true;
                }
                receiver = index.object();
            }
            Expr::Call(call) => {
                if state.is_function_expression_invocation(call.callee()) {
                    return false;
                }
                receiver = call.callee();
            }
            _ => return false,
        }
    }
}

fn is_iterable<'a>(receiver: Expr<'a>, state: &SemanticState<'a>) -> bool {
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

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Call(call) = node
            && let Expr::Field(field) = call.callee()
            && !field.is_null_safe()
            && field.field().name() == "forEach"
            && matches!(call.args().positional().next(), Some(Expr::FuncExpr(_)))
            && !state.inside_cascade
            && !state.is_function_expression_invocation(call.callee())
            && !has_method_chain(field.object(), state)
            && !has_null_aware_chain(field.object(), state)
            && is_iterable(field.object(), state)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-function-literals-in-foreach-calls",
                "Avoid using a function literal in a 'forEach' call; use a for-in loop instead.",
                field.field().span(),
            ));
        }
    }
}
