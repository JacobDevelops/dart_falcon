//! Flags empty `catch` blocks.
//!
//! A catch clause with no body silently swallows the exception, hiding failures
//! that should be handled, logged, or rethrown and making bugs far harder to
//! diagnose. Handle the error, or at minimum log it, rather than discarding it.
//! Two escape hatches match the official lint: a catch whose body contains a
//! comment (the emptiness is deliberate and documented), and a catch that binds
//! its exception to `_`, an explicit "I am ignoring this" marker.
//!
//! This includes catch clauses inside anonymous functions.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct EmptyCatches;

impl Rule for EmptyCatches {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

fn check_catch(catch: CatchClause<'_>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Exempt `catch (_) {}` / `on E catch (_) {}`.
    if catch.exception_var().is_some_and(|v| v.name() == "_") {
        return;
    }
    let Some(span) = crate::lint::support::empty_blocks::empty_block_end(catch.body()) else {
        return;
    };
    diags.push(ctx.diagnostic("Handle the exception, add a comment, or name it `_` instead of leaving this catch block empty.", span));
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
        if let Stmt::TryCatch(statement) = node {
            for catch in statement.catches() {
                check_catch(catch, &mut self.diags, self.ctx);
            }
        }
        StructuralTraversal::Continue
    }
}
}
