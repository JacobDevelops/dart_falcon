//! Flags stray empty statements (a lone `;`).
//!
//! A solitary semicolon does nothing, and where it slips in it usually changes
//! meaning: `while (cond);` turns the following block into an unconditional body,
//! and a stray `;` after an `if` header detaches the intended branch. It is
//! almost always an accidental extra semicolon or a loop body that should have
//! been a block. Remove it, or replace it with the block that was intended. The
//! semicolons in a `for (;;)` header are loop syntax, not statements, and are
//! never reported. A real `null;` expression statement is not an empty statement.
//!
//! This includes empty statements inside anonymous functions.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct EmptyStatements;

impl Rule for EmptyStatements {
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
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Empty(empty) = node {
            self.diags.push(self.ctx.diagnostic(
                "Remove the unnecessary `;` or replace it with a block.",
                empty.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
