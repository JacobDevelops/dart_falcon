//! Flags an `if` returning a boolean literal followed by the opposite literal.
//!
//! Return the condition (or its negation) instead of spelling out both boolean
//! outcomes. Only adjacent statements with a plain condition, no `else`, and
//! one return in the then-branch are checked. Pattern conditions, extra branch
//! work, non-literal results, and equal results are left alone. Explicit `else`
//! branches are left to `prefer-conditional-expressions`. The fallback return
//! is highlighted; evaluation order and short-circuit behavior are unchanged.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{body_list, member_list, stmt_lists};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferReturningCondition;

impl Rule for PreferReturningCondition {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

fn returned_boolean(mut statement: Stmt<'_>) -> Option<bool> {
    while let Stmt::Block(block) = statement {
        if block.stmts().len() != 1 {
            return None;
        }
        statement = block.stmts().first()?;
    }
    let Stmt::Return(returned) = statement else {
        return None;
    };
    let Expr::BoolLit(literal) = returned.value()?.unparenthesized() else {
        return None;
    };
    Some(literal.value())
}

impl Collector<'_> {
    fn check(&mut self, statements: AstChildren<'_, Stmt<'_>>) {
        for (previous, current) in statements.clone().zip(statements.skip(1)) {
            if let Stmt::If(condition) = previous
                && condition.else_branch().is_none()
                && matches!(condition.condition(), IfCondition::Expr(_))
                && let Some(branch) = returned_boolean(condition.then_branch())
                && let Some(fallback) = returned_boolean(current)
                && matches!(current, Stmt::Return(_))
                && branch != fallback
            {
                self.diags.push(self.ctx.diagnostic("Return the condition or its negation instead of boolean literals.", current.span()));
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        body_list(node.body(), &mut |statements, _| self.check(statements));
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        member_list(node, &mut |statements, _| self.check(statements));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        stmt_lists(node, &mut |statements, _| self.check(statements));
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            body_list(Some(function.body()), &mut |statements, _| self.check(statements));
        }
        StructuralTraversal::Continue
    }
}
}
