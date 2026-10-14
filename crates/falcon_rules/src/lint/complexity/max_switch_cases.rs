//! Flags a `switch` statement with more than the configured number of cases.
//!
//! A switch with many cases is often better modeled as a map lookup,
//! polymorphism, or a sealed type with exhaustive handling. The rule counts
//! non-default (pattern) cases — the `default` clause is not counted — and
//! reports at the switch when the count exceeds the threshold. Nested switches
//! are checked independently.
//!
//! ## Options
//!
//! `max_cases` (integer, default: 10) — flag when the number of non-default
//! cases exceeds this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct MaxSwitchCases;

impl Rule for MaxSwitchCases {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            threshold: max_cases_option(ctx),
        };
        collector.visit_program(program);
        collector.diags
    }
}

fn count_non_default_cases(switch_stmt: &SwitchStmt) -> usize {
    switch_stmt
        .cases
        .iter()
        .flat_map(|case| &case.cases)
        .filter(|kind| matches!(kind, SwitchCaseKind::Pattern(..)))
        .count()
}

/// Read the `max_cases` option (default 10). Malformed/missing → default.
fn max_cases_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "max-switch-cases"),
        "max_cases",
    )
    .unwrap_or(10)
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    threshold: usize,
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Switch(switch_stmt) = node
            && count_non_default_cases(switch_stmt) > self.threshold
        {
            self.diags.push(self.ctx.diagnostic(
                "max-switch-cases",
                format!(
                    "Switch statement has too many cases (max {}).",
                    self.threshold
                ),
                switch_stmt.span,
            ));
        }
        walk_stmt(self, node);
    }
}
