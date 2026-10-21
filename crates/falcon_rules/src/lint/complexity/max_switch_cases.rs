//! Flags a `switch` statement with more than the configured number of cases.
//!
//! A switch with many cases is often better modeled as a map lookup,
//! polymorphism, or a sealed type with exhaustive handling. The rule counts
//! non-default cases — the `default` clause is not counted — and
//! reports at the switch when the count exceeds the threshold. Nested switches
//! are checked independently.
//!
//! ## Options
//!
//! `max_cases` (integer, default: 10) — flag when the number of non-default
//! cases exceeds this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MaxSwitchCases;

impl Rule for MaxSwitchCases {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            threshold: max_cases_option(ctx),
        }))
    }
}

fn count_non_default_cases(switch_stmt: SwitchStmt<'_>) -> usize {
    switch_stmt
        .cases()
        .flat_map(|case| case.cases())
        .filter(|kind| matches!(kind, SwitchCaseKind::Pattern(..) | SwitchCaseKind::Expr(..)))
        .count()
}

/// Read the `max_cases` option (default 10). Malformed/missing → default.
fn max_cases_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, MaxSwitchCases::NAME),
        "max_cases",
    )
    .unwrap_or(10)
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    threshold: usize,
}

falcon_analyze::structural_visitor! {
impl<'a, 'ctx> StructuralRuleVisitor<'ctx> for Collector<'a, 'ctx> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'ctx>) -> StructuralTraversal {
        if let Stmt::Switch(switch_stmt) = node
            && count_non_default_cases(switch_stmt) > self.threshold
        {
            self.diags.push(self.ctx.diagnostic(
                format!(
                    "Switch statement has too many cases (max {}).",
                    self.threshold
                ),
                switch_stmt.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
