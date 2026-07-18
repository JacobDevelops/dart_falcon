//! Flags files longer than the configured line limit. Ported from pyramid_lint's `max_lines_for_file`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct MaxLinesForFile;

/// Read the `max_lines` option (default 200). Malformed/missing → default.
fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("max_lines_for_file")
        .and_then(|m| ctx.rule_options(m.group, "max_lines_for_file"))
        .and_then(|o| o.get("max_lines"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(200)
}

impl Rule for MaxLinesForFile {
    fn name(&self) -> &'static str {
        "max_lines_for_file"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Count total lines in the file
        let line_count = ctx.source.lines().count();
        let threshold = max_lines_option(ctx);

        if line_count > threshold {
            // Report on line 1 (byte offset 0). Message states the actual threshold.
            diags.push(Diagnostic::new(
                "max_lines_for_file",
                Severity::Warning,
                format!("File exceeds the maximum number of lines ({threshold})."),
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: 0, end: 0 },
            ));
        }

        diags
    }
}
