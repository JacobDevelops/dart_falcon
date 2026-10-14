//! Enforces `lowercase_with_underscores` for source file names.

use crate::lint::naming::is_snake_case;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::Program;

#[derive(Default)]
pub struct FileNames;

impl Rule for FileNames {
    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(file_name) = ctx.file_path.file_name().and_then(|name| name.to_str()) else {
            return Vec::new();
        };
        let Some(stem) = file_name.strip_suffix(".dart") else {
            return Vec::new();
        };
        if stem.split('.').all(is_snake_case) {
            return Vec::new();
        }
        vec![ctx.diagnostic(
            "file-names",
            "Name source files using lowercase_with_underscores.",
            DiagSpan { start: 0, end: 0 },
        )]
    }
}
