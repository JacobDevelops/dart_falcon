//! Flags `x == null ? null : x.y` (and the inverted `x != null ? x.y : null`).
//!
//! Guarding a member access with a null-check ternary is what the `?.`
//! null-aware operator does: `x?.y`. It is shorter and evaluates the receiver
//! only once. The rule fires when one branch is the `null` literal and the
//! other is a field access, index, or method call whose receiver matches the
//! checked operand (whitespace-insensitive).

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferNullAwareOperators;

impl Rule for PreferNullAwareOperators {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

use crate::lint::expressions::{equivalent, null_check};

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

/// The receiver of a direct member access, index, or method call — the part
/// that `?.` would guard.
fn access_receiver(expr: &Expr) -> Option<&Expr> {
    match expr.unparenthesized() {
        Expr::Field { object, .. } | Expr::Index { object, .. } => Some(object),
        Expr::Call { callee, .. } => match callee.unparenthesized() {
            Expr::Field { object, .. } | Expr::Index { object, .. } => Some(object),
            _ => None,
        },
        _ => None,
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            span,
        } = node
            && let Some((operand, is_eq)) = null_check(condition)
        {
            // `x == null ? null : x.y` — null branch is `then`, access is `else`;
            // `x != null ? x.y : null` — access is `then`, null branch is `else`.
            let (null_branch, access_branch) = if is_eq {
                (then_expr, else_expr)
            } else {
                (else_expr, then_expr)
            };
            if matches!(null_branch.unparenthesized(), Expr::NullLit { .. })
                && let Some(receiver) = access_receiver(access_branch)
                && equivalent(receiver, operand, self.ctx)
            {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-null-aware-operators",
                    "Prefer using the null-aware operator '?.'.",
                    *span,
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
