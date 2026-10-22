//! Flags an unlabeled continue at the end of a loop body or its final branch.
//!
//! Ancestor inspection stops at try/finally, switch and function boundaries.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryContinue;

impl Rule for AvoidUnnecessaryContinue {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Continue(statement) = node
            && statement.label().is_none()
            && is_loop_tail(node)
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Remove this trailing continue; the loop already advances here.",
                statement.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

fn is_loop_tail(statement: Stmt<'_>) -> bool {
    let mut child = statement.syntax();
    while let Some(parent) = child.parent() {
        match Stmt::cast(parent) {
            Some(Stmt::Block(block)) => {
                if block
                    .stmts()
                    .last()
                    .is_none_or(|last| last.syntax().id() != child.id())
                {
                    return false;
                }
            }
            Some(Stmt::If(_)) => {}
            Some(Stmt::For(statement)) => return statement.body().syntax().id() == child.id(),
            Some(Stmt::While(statement)) => return statement.body().syntax().id() == child.id(),
            Some(Stmt::DoWhile(statement)) => return statement.body().syntax().id() == child.id(),
            _ => return false,
        }
        child = parent;
    }
    false
}
