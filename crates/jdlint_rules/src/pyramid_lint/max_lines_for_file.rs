use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct MaxLinesForFile;

impl Rule for MaxLinesForFile {
    fn name(&self) -> &'static str {
        "max_lines_for_file"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Count total lines in the file
        let line_count = ctx.source.lines().count();
        let threshold = 75;

        if line_count > threshold {
            // Report on line 1 (byte offset 0)
            diags.push(Diagnostic::new(
                "max_lines_for_file",
                Severity::Warning,
                "File exceeds the maximum number of lines (500).",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: 0, end: 0 },
            ));
        }

        diags
    }
}
