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
//! `max-cases` (integer, default: 10) — flag when the number of non-default
//! cases exceeds this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum number of non-default cases in a switch.
    #[serde(deserialize_with = "crate::lint::options::integer")]
    max_cases: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_cases: 10 }
    }
}

crate::lint::options::impl_options!(Options);

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

fn max_cases_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::get::<Options>(ctx, MaxSwitchCases::NAME).max_cases
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
