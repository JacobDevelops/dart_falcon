//! Flags an if branch that only returns the same expression as the following return.
//!
//! The if must have no else and its body must be a single value-return statement.
//! Expressions are compared by token spelling, preserving literal contents. Check
//! whether the branch should return something different; when removing it, retain
//! any side effects in the condition. Bare returns belong to avoid-unnecessary-return.
//! If-case patterns are excluded because their bindings can shadow return values.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryIf;

impl Rule for AvoidUnnecessaryIf {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_block(&mut self, block: Block<'a>) -> StructuralTraversal {
            let mut statements = block.stmts();
            let mut previous = statements.next();
            for statement in statements {
                if let Some(Stmt::If(branch)) = previous
                    && branch.else_branch().is_none()
                    && matches!(branch.condition(), IfCondition::Expr(_))
                    && let Stmt::Return(after) = statement
                {
                    let body = match branch.then_branch() {
                        Stmt::Block(body) if body.stmts().len() == 1 => body.stmts().first(),
                        Stmt::Return(ret) => Some(Stmt::Return(ret)),
                        _ => None,
                    };
                    if let Some(Stmt::Return(inside)) = body
                        && let (Some(inside), Some(after)) = (inside.value(), after.value())
                        && crate::lint::expressions::equivalent(inside.unparenthesized(), after.unparenthesized(), self.ctx)
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "This branch returns the same value as the following return; check or remove the redundant branch.",
                            branch.span(),
                        ));
                    }
                }
                previous = Some(statement);
            }
        StructuralTraversal::Continue
    }
}
}
