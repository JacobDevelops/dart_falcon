//! Flags an `else` clause whose body is an empty statement (`else ;`).
//!
//! An `else` immediately followed by a semicolon binds that empty statement as
//! the whole else branch, so the block meant to run instead executes
//! unconditionally right after the `if`. This is nearly always a stray semicolon
//! typed between `else` and the intended `{ ... }`. Remove the semicolon so the
//! following block becomes the else body, or delete the `else` entirely if no
//! alternative branch is needed.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidEmptyElse;

impl Rule for AvoidEmptyElse {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(if_stmt) = node
            && let Some(else_branch) = if_stmt.else_branch()
            && matches!(else_branch, Stmt::Empty(_))
        {
            let span = else_branch.span();
            self.diags.push(self.ctx.diagnostic(
                "Remove the empty `else` clause or give it a body.",
                span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
