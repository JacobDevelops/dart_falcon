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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use crate::lint::expressions::{is_parenthesized, paren_text};

#[derive(Default)]
pub struct PreferNullAwareMethodCalls;

impl Rule for PreferNullAwareMethodCalls {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
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
        let source = self.ctx.source;
        if is_parenthesized(call) {
            return;
        }
        let Expr::Call { callee, span, .. } = call else {
            return;
        };
        let Expr::NullAssert { operand, .. } = callee.as_ref() else {
            return;
        };
        if !is_parenthesized(callee) && paren_text(operand, source) == paren_text(tested, source) {
            self.diags.push(self.ctx.diagnostic(
                "prefer-null-aware-method-calls",
                "Use a null-aware invocation of the 'call' method rather than explicitly testing for null.",
                *span,
            ));
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
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
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
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
        walk_expr(self, node);
    }
}
