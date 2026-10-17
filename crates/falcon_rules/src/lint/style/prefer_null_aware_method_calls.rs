//! Prefers `f?.call()` to calling a null-checked function after a `!= null` test.
//!
//! `if (f != null) f!();` and `f != null ? f!() : null` both invoke a nullable
//! function only when it is non-null; `f?.call()` says the same thing directly.
//! The rule fires when the condition is `<expr> != null` (null on the right), the
//! `if` has no `else` and its then-branch is that single call (optionally in a
//! block), or the conditional's else branch is `null`, and the call's function
//! is `<expr>!` with `<expr>` textually equal to the tested expression.
//! Parenthesized conditions, nulls, calls and callees do not match; compared
//! operands retain their parentheses, as in the official rule.
//!
//! Adopted from the official `prefer_null_aware_method_calls` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::expressions::{equivalent, is_parenthesized};

#[derive(Default)]
pub struct PreferNullAwareMethodCalls;

impl Rule for PreferNullAwareMethodCalls {
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

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    /// The tested expression of `<expr> != null`, without a wrapped null.
    fn not_null_operand<'e>(&self, condition: &'e Expr) -> Option<&'e Expr> {
        match condition {
            Expr::Binary {
                op: BinaryOp::NotEq,
                left,
                right,
                ..
            } if matches!(right.as_ref(), Expr::NullLit { .. }) && !is_parenthesized(right) => {
                Some(left)
            }
            _ => None,
        }
    }
    fn check(&mut self, call: &Expr, tested: &Expr) {
        if is_parenthesized(call) {
            return;
        }
        let Expr::Call { callee, span, .. } = call else {
            return;
        };
        let Expr::NullAssert { operand, .. } = callee.as_ref() else {
            return;
        };
        if !is_parenthesized(callee) && equivalent(operand, tested, self.ctx) {
            self.diags.push(self.ctx.diagnostic(
                "prefer-null-aware-method-calls",
                "Use a null-aware invocation of the 'call' method rather than explicitly testing for null.",
                *span,
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::If(IfStmt {
            condition: IfCondition::Expr(condition),
            then_branch,
            else_branch: None,
            ..
        }) = node
            && !matches!(condition, Expr::Paren { .. })
            && let Some(tested) = self.not_null_operand(condition)
        {
            let then = match then_branch.as_ref() {
                Stmt::Block(block) if block.stmts.len() == 1 => &block.stmts[0],
                other => other,
            };
            if let Stmt::Expr(statement) = then {
                self.check(&statement.expr, tested);
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } = node
            && matches!(else_expr.as_ref(), Expr::NullLit { .. })
            && !is_parenthesized(else_expr)
            && !is_parenthesized(condition)
            && let Some(tested) = self.not_null_operand(condition)
        {
            self.check(then_expr, tested);
        }
        StructuralTraversal::Continue
    }
}
}
