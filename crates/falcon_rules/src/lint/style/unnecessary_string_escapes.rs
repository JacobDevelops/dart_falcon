//! Flags a backslash escape that has no effect on the string's value.
//!
//! A backslash only matters before a character that is special in the current
//! string context: recognized escapes (`\n`, `\t`, `\x`, `\u`, and friends), the
//! active quote character, `$`, and `\` itself. Escaping anything else — `\a`, or
//! `\'` inside a double-quoted string — produces the same character while adding
//! visual clutter and inviting confusion about whether an escape was intended.
//! Raw strings (`r'...'`) are never analyzed since backslashes are literal there,
//! and triple-quoted strings are skipped to avoid the edge cases around escaping
//! runs of quotes. Remove the redundant backslash.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_program};

pub struct UnnecessaryStringEscapes;

impl Rule for UnnecessaryStringEscapes {
    fn name(&self) -> &'static str {
        "unnecessary-string-escapes"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// A single-segment string literal decomposed into its delimiter and inner content.
struct StrLit<'a> {
    is_raw: bool,
    quote: u8,
    triple: bool,
    content: &'a str,
    content_offset: usize,
}

fn parse_str_lit(raw: &str) -> Option<StrLit<'_>> {
    let is_raw = raw.as_bytes().first() == Some(&b'r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let (triple, quote, dlen) = if rest.starts_with("'''") {
        (true, b'\'', 3)
    } else if rest.starts_with("\"\"\"") {
        (true, b'"', 3)
    } else if rest.starts_with('\'') {
        (false, b'\'', 1)
    } else if rest.starts_with('"') {
        (false, b'"', 1)
    } else {
        return None;
    };
    let closing = &rest[..dlen];
    if rest.len() < 2 * dlen || !rest[dlen..].ends_with(closing) {
        return None;
    }
    let content_offset = prefix + dlen;
    let content = &raw[content_offset..raw.len() - dlen];
    Some(StrLit {
        is_raw,
        quote,
        triple,
        content,
        content_offset,
    })
}

/// Escape sequences that change the string's value, so their backslash is required.
fn is_meaningful_escape(c: u8, quote: u8) -> bool {
    matches!(c, b'n' | b'r' | b't' | b'b' | b'f' | b'v' | b'x' | b'u')
        || c == quote
        || c == b'$'
        || c == b'\\'
        || c == b'\n'
        || c == b'\r'
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_string_lit(&mut self, node: &StringLitNode) {
        let Some(lit) = parse_str_lit(&node.raw) else {
            return;
        };
        if lit.is_raw || lit.triple {
            return;
        }
        let bytes = lit.content.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' if i + 1 < bytes.len() => {
                    let c = bytes[i + 1];
                    if !is_meaningful_escape(c, lit.quote) {
                        let start = node.span.start + lit.content_offset + i;
                        self.diags.push(Diagnostic::new(
                            "unnecessary-string-escapes",
                            Severity::Warning,
                            "Unnecessary escape; the backslash can be removed.",
                            self.file.clone(),
                            DiagSpan {
                                start,
                                end: start + 2,
                            },
                        ));
                    }
                    i += 2;
                }
                // An unescaped delimiter means adjacent-literal concatenation merged the
                // segments; offsets past here are unreliable, so stop.
                q if q == lit.quote => break,
                _ => i += 1,
            }
        }
    }
}
