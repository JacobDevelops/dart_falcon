//! Flags a file longer than the configured line limit.
//!
//! Very long files are hard to navigate and usually mix unrelated
//! responsibilities; splitting them improves readability and review. The rule
//! counts the total lines in the file and reports once, at the top, when the
//! count exceeds the threshold. The diagnostic message states the configured
//! limit.
//!
//! ## Options
//!
//! `max_lines` (integer, default: 200) — flag files with more than this many
//! lines.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct MaxLinesForFile;

/// Read the `max_lines` option (default 200). Malformed/missing → default.
fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "max-lines-for-file"),
        "max_lines",
    )
    .unwrap_or(200)
}

impl Rule for MaxLinesForFile {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();

        let line_count = ctx.source.lines().count();
        let threshold = max_lines_option(ctx);

        if line_count > threshold {
            // Report on line 1 (byte offset 0). Message states the actual threshold.
            diags.push(ctx.diagnostic(
                "max-lines-for-file",
                format!("File exceeds the maximum number of lines ({threshold})."),
                DiagSpan { start: 0, end: 0 },
            ));
        }

        RulePreparation::Diagnostics(diags)
    }
}
