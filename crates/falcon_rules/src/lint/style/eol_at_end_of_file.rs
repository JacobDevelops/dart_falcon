//! Flags a non-empty file that does not end with exactly one line break.
//!
//! A missing final newline breaks tools that process files line by line, and
//! extra blank lines at the end are noise. Both are reported at the end of the
//! last non-whitespace character:
//!
//! ```text
//! void f() {}⏎        // fine
//! void f() {}         // flagged: no newline at the end
//! void f() {}⏎⏎       // flagged: more than one newline at the end
//! ```
//!
//! Adopted from the official `eol_at_end_of_file` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::Program;

#[derive(Default)]
pub struct EolAtEndOfFile;

impl Rule for EolAtEndOfFile {
    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let source = ctx.source;
        if source.is_empty() {
            return Vec::new();
        }
        let message = if ["\n\n", "\r\r", "\r\n\r\n"]
            .iter()
            .any(|ending| source.ends_with(ending))
        {
            "Multiple newlines at the end of the file."
        } else if !source.ends_with(['\n', '\r']) {
            "Missing a newline at the end of the file."
        } else {
            return Vec::new();
        };
        let start = source.trim_end().len();
        let end = source[start..]
            .chars()
            .next()
            .map_or(start, |c| start + c.len_utf8());
        vec![ctx.diagnostic("eol-at-end-of-file", message, DiagSpan { start, end })]
    }
}
