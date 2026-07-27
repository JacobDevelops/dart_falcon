//! Enforces `lowercase_with_underscores` for source file names.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;

pub struct FileNames;

impl Rule for FileNames {
    fn name(&self) -> &'static str {
        "file-names"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(file_name) = ctx.file_path.file_name().and_then(|name| name.to_str()) else {
            return Vec::new();
        };
        let Some(stem) = file_name.strip_suffix(".dart") else {
            return Vec::new();
        };
        if stem.split('.').all(valid_name) {
            return Vec::new();
        }
        vec![Diagnostic::new(
            "file-names",
            Severity::Warning,
            "Name source files using lowercase_with_underscores.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan { start: 0, end: 0 },
        )]
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}
