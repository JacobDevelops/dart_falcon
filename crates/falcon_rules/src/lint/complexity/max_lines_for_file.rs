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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct MaxLinesForFile;

/// Read the `max_lines` option (default 200). Malformed/missing → default.
fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("max-lines-for-file")
        .and_then(|m| ctx.rule_options(m.group, "max-lines-for-file"))
        .and_then(|o| o.get("max_lines"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(200)
}

impl Rule for MaxLinesForFile {
    fn name(&self) -> &'static str {
        "max-lines-for-file"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        let line_count = ctx.source.lines().count();
        let threshold = max_lines_option(ctx);

        if line_count > threshold {
            // Report on line 1 (byte offset 0). Message states the actual threshold.
            diags.push(Diagnostic::new(
                "max-lines-for-file",
                Severity::Warning,
                format!("File exceeds the maximum number of lines ({threshold})."),
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: 0, end: 0 },
            ));
        }

        diags
    }
}
