//! Stable constants without runtime allocations or implicit extension receivers.

use crate::lint::const_eval::{constant_in_context, deferred_prefixes};
use falcon_analyze::SemanticState;
use falcon_analyze::{AnalyzeContext, ConstantValue};
use falcon_analyze::{ConstantContext, evaluate_constant};
use falcon_syntax::cst::visit::walk_expr_chain;
use falcon_syntax::cst::*;
pub(crate) fn constant_value(
    expression: Expr<'_>,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> Option<ConstantValue> {
    // An extension's implicit receiver is its on-type, not the extension's
    // lexical member namespace. In particular, unnamed extension members have
    // no declaration identity for the constant evaluator to inspect.
    if state.enclosing_type.is_some()
        && let Some(extension) = expression
            .syntax()
            .ancestors()
            .find_map(ExtensionDecl::cast)
    {
        struct InstanceReferences<'a> {
            extension: ExtensionDecl<'a>,
            found: bool,
        }
        impl<'a> falcon_syntax::cst::visit::Visitor<'a> for InstanceReferences<'_> {
            fn visit_expr(&mut self, node: Expr<'a>) {
                walk_expr_chain(self, node, |visitor, node| {
                    if let Expr::Ident(identifier) = node {
                        visitor.found |= visitor.extension.members().any(|member| match member {
                            ClassMember::Getter(getter) => {
                                !getter.is_static() && getter.name().name() == identifier.name()
                            }
                            ClassMember::Method(method) => {
                                !method.is_static() && method.name().name() == identifier.name()
                            }
                            _ => false,
                        });
                    }
                    !visitor.found
                });
            }
        }
        let mut references = InstanceReferences {
            extension,
            found: false,
        };
        falcon_syntax::cst::visit::Visitor::visit_expr(&mut references, expression);
        if references.found {
            return None;
        }
    }
    if has_runtime_allocation(expression, state, ctx) {
        return None;
    }
    if !constant_in_context(
        expression,
        &deferred_prefixes(ctx.parsed().program()),
        state,
    ) {
        return None;
    }
    evaluate_constant(
        expression,
        ConstantContext {
            owner: state.constant_owner.as_ref(),
            locals: Some(&state.visible_local_constants()),
            stable: true,
            ..ConstantContext::new(&state.model, &state.signatures)
        },
    )
}

fn has_runtime_allocation(
    expression: Expr<'_>,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    struct Allocations<'s, 'a> {
        state: &'s SemanticState<'a>,
        found: bool,
    }
    impl<'a> falcon_syntax::cst::visit::Visitor<'a> for Allocations<'_, 'a> {
        fn visit_expr(&mut self, node: Expr<'a>) {
            walk_expr_chain(self, node, |visitor, node| {
                let allocation = match node {
                    Expr::List(node) => Some(node.is_const()),
                    Expr::Map(node) => Some(node.is_const()),
                    Expr::Set(node) => Some(node.is_const()),
                    Expr::Record(node) => Some(node.is_const()),
                    Expr::New(node) => Some(node.is_const()),
                    Expr::Call(call)
                        if matches!(call.callee().unparenthesized(),
                            Expr::DotShorthand(node) if node.is_const()) =>
                    {
                        Some(true)
                    }
                    Expr::Call(call)
                        if falcon_analyze::resolve_call_constructor(
                            call.callee(),
                            &visitor.state.model,
                            true,
                        )
                        .is_some() =>
                    {
                        Some(false)
                    }
                    _ => None,
                };
                match allocation {
                    Some(false) => {
                        visitor.found = true;
                        false
                    }
                    // Explicit const allocations also make their contents constant.
                    Some(true) => false,
                    None => !visitor.found,
                }
            });
        }
    }
    let mut allocations = Allocations {
        state,
        found: false,
    };
    falcon_syntax::cst::visit::Visitor::visit_expr(&mut allocations, expression);
    allocations.found
        && !ctx
            .const_contexts(ctx.parsed().program())
            .contains(&expression.span())
}
