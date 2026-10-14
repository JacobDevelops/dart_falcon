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

use crate::lint::string_lit::interpolated_literals;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct UnnecessaryStringEscapes;

impl Rule for UnnecessaryStringEscapes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
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

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in interpolated_literals(node) {
            if lit.triple {
                continue;
            }
            let bytes = lit.content.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' if i + 1 < bytes.len() => {
                        let c = bytes[i + 1];
                        if !is_meaningful_escape(c, lit.quote) {
                            let start = span.start + lit.content_offset + i;
                            self.diags.push(self.ctx.diagnostic(
                                "unnecessary-string-escapes",
                                "Unnecessary escape; the backslash can be removed.",
                                DiagSpan {
                                    start,
                                    end: start + 2,
                                },
                            ));
                        }
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
        }
    }
}
