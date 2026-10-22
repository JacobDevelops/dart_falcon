//! Flags an else-free `if` whose only then-statement is another else-free `if`.
//!
//! Combining these conditions with `&&` removes unnecessary nesting without
//! changing evaluation order. Both conditions must be ordinary expressions;
//! pattern conditions, either `else` branch, and blocks containing other
//! statements are excluded. No automatic edit is made, preserving comments.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidCollapsibleIf;

impl Rule for AvoidCollapsibleIf {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, CollapsibleIf, ctx, diags)
    }
}

struct CollapsibleIf<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for CollapsibleIf<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(outer) = node
            && outer.else_branch().is_none()
            && matches!(outer.condition(), IfCondition::Expr(_))
        {
            let mut branch = outer.then_branch();
            while let Stmt::Block(block) = branch {
                let mut statements = block.stmts();
                match (statements.next(), statements.next()) {
                    (Some(only), None) => branch = only,
                    _ => break,
                }
            }
            if let Stmt::If(inner) = branch
                && inner.else_branch().is_none()
                && let IfCondition::Expr(condition) = inner.condition()
            {
                self.diags.push(self.ctx.diagnostic("Merge this nested if with its outer condition using &&.", condition.span()));
            }
        }
        StructuralTraversal::Continue
    }
}
}
