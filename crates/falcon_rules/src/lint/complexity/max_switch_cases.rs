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
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

pub struct MaxSwitchCases;

impl Rule for MaxSwitchCases {
    fn name(&self) -> &'static str {
        "max-switch-cases"
    }

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
    crate::meta::meta_for("max-switch-cases")
        .and_then(|m| ctx.rule_options(m.group, "max-switch-cases"))
        .and_then(|o| o.get("max_cases"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
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
            self.diags.push(Diagnostic::new(
                "max-switch-cases",
                Severity::Warning,
                format!(
                    "Switch statement has too many cases (max {}).",
                    self.threshold
                ),
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: switch_stmt.span.start,
                    end: switch_stmt.span.end,
                },
            ));
        }
        walk_stmt(self, node);
    }
}
