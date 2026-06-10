use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct FormatComment;

impl Rule for FormatComment {
    fn name(&self) -> &'static str {
        "format-comment"
    }

    fn analyze(&self, _program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        scan_source(ctx.source, &mut diags, ctx);
        diags
    }
}

fn scan_source(source: &str, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            // Block comment — skip entirely
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            // Line comment — check format
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                let comment_start = i;
                i += 2;
                // Skip optional leading spaces
                while i < len && bytes[i] == b' ' {
                    i += 1;
                }
                if i < len && bytes[i] != b'\n' {
                    let first_char = bytes[i] as char;
                    if first_char.is_ascii_lowercase() {
                        diags.push(Diagnostic::new(
                            "format-comment",
                            Severity::Warning,
                            "Comments should start with an uppercase letter",
                            ctx.file_path.to_string_lossy().into_owned(),
                            DiagSpan { start: comment_start, end: comment_start + 2 },
                        ));
                    }
                }
                // Advance to end of line
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            // Double-quoted string — skip contents
            b'"' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            // Single-quoted string — skip contents
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
}
