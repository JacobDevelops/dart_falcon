//! Flags a string literal that escapes its own quote character when switching
//! the outer quotes would remove every escape.
//!
//! A single-quoted literal whose value contains `'` but no `"` (or the mirror
//! case for double quotes) can swap its delimiters and drop the backslashes:
//!
//! ```dart
//! var s = 'It\'s here';   // flagged
//! var s = "It's here";    // preferred
//! var s = 'It\'s "here"'; // fine: both quote kinds appear
//! ```
//!
//! Only literal text counts; interpolated expressions are ignored. Raw and
//! multiline (triple-quoted) literals are never flagged. Each part of an
//! adjacent-string concatenation is checked on its own.
//!
//! Adopted from the official `avoid_escaping_inner_quotes` lint.

use crate::lint::string_lit::{decoded_text, literal_parts};
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct AvoidEscapingInnerQuotes;

impl Rule for AvoidEscapingInnerQuotes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in node.segments() {
            if !lit.terminated || lit.is_raw || lit.triple {
                continue;
            }
            let (own, other) = if lit.quote == b'\'' {
                ('\'', '"')
            } else {
                ('"', '\'')
            };
            let text = decoded_text(&lit, &literal_parts(&lit));
            if text.contains(own) && !text.contains(other) {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-escaping-inner-quotes",
                    format!("Unnecessary escape of '{own}'; change the outer quotes to '{other}'."),
                    span,
                ));
            }
        }
    }
}
