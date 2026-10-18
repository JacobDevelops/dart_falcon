//! Reports long if/else-if chains; replace them with a switch or extracted helpers.
//!
//! `max_branches` (integer, default: 3) counts every if and the final else.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidIfWithManyBranches;
impl Rule for AvoidIfWithManyBranches {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            threshold: crate::lint::options::usize(
                crate::lint::options::for_rule(ctx, "avoid-if-with-many-branches"),
                "max_branches",
            )
            .unwrap_or(3),
            chained: None,
            parents: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    threshold: usize,
    chained: Option<usize>,
    parents: Vec<Option<usize>>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.parents.push(self.chained);
        if let Stmt::If(statement) = node {
            let is_chained = self.chained == Some(statement.span().start);
            self.chained = statement.else_branch().and_then(|branch| match branch {
                Stmt::If(next) => Some(next.span().start),
                _ => None,
            });
            if !is_chained {
                let mut count = 1;
                let mut branch = statement.else_branch();
                while let Some(Stmt::If(next)) = branch {
                    count += 1;
                    branch = next.else_branch();
                }
                count += usize::from(branch.is_some());
                if count > self.threshold {
                    self.diags.push(self.ctx.diagnostic("avoid-if-with-many-branches", format!("If chain has {count} branches (max {}). Replace it with a switch or extracted helpers.", self.threshold), statement.span()));
                }
            }
        }
        StructuralTraversal::Continue
    }
    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        self.chained = self.parents.pop().flatten();
    }
}
}
