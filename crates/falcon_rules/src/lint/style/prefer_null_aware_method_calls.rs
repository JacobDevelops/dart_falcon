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
use falcon_syntax::cst::*;

use crate::lint::expressions::{equivalent, is_parenthesized};

#[derive(Default)]
pub struct PreferNullAwareMethodCalls;

impl Rule for PreferNullAwareMethodCalls {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

impl<'a> Collector<'a> {
    /// The tested expression of `<expr> != null`, without a wrapped null.
    fn not_null_operand(&self, condition: Expr<'a>) -> Option<Expr<'a>> {
        match condition {
            Expr::Binary(binary)
                if binary.op() == BinaryOp::NotEq
                    && matches!(binary.right(), Expr::NullLit(_))
                    && !is_parenthesized(binary.right()) =>
            {
                Some(binary.left())
            }
            _ => None,
        }
    }
    fn check(&mut self, call: Expr<'a>, tested: Expr<'a>) {
        if is_parenthesized(call) {
            return;
        }
        let Expr::Call(invocation) = call else {
            return;
        };
        let callee = invocation.callee();
        let Expr::NullAssert(assertion) = callee else {
            return;
        };
        if !is_parenthesized(callee) && equivalent(assertion.operand(), tested, self.ctx) {
            self.diags.push(self.ctx.diagnostic("Use a null-aware invocation of the `call` method rather than explicitly testing for null.", invocation.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(statement) = node
            && statement.else_branch().is_none()
            && let IfCondition::Expr(condition) = statement.condition()
            && !matches!(condition, Expr::Paren(_))
            && let Some(tested) = self.not_null_operand(condition)
        {
            let then = match statement.then_branch() {
                Stmt::Block(block) if block.stmts().len() == 1 => block.stmts().first().unwrap(),
                other => other,
            };
            if let Stmt::Expr(statement) = then {
                self.check(statement.expr(), tested);
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Conditional(conditional) = node
            && matches!(conditional.else_expr(), Expr::NullLit(_))
            && !is_parenthesized(conditional.else_expr())
            && !is_parenthesized(conditional.condition())
            && let Some(tested) = self.not_null_operand(conditional.condition())
        {
            self.check(conditional.then_expr(), tested);
        }
        StructuralTraversal::Continue
    }
}
}
