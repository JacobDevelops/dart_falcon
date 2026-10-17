//! Flags backslashes that do not affect a string's value.
//!
//! Raw strings are exempt. In triple-quoted strings, escaping the delimiter
//! quote is only necessary in runs of three quotes or at the closing delimiter.

use crate::lint::string_lit::{
    LiteralPart, interpolated_literals, literal_parts, multiline_lead_len,
};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryStringEscapes;

impl Rule for UnnecessaryStringEscapes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

fn is_meaningful_escape(c: u8, quote: u8) -> bool {
    matches!(c, b'n' | b'r' | b't' | b'b' | b'f' | b'v' | b'x' | b'u')
        || c == quote
        || c == b'$'
        || c == b'\\'
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn report(&mut self, start: usize) {
        self.diags.push(self.ctx.diagnostic(
            "unnecessary-string-escapes",
            "Unnecessary escape; the backslash can be removed.",
            DiagSpan {
                start,
                end: start + 1,
            },
        ));
    }

    fn check_text(&mut self, text: &str, start: usize, lit: StringDelimiter<'_>, final_part: bool) {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if lit.triple
                && (bytes[i] == lit.quote
                    || (bytes[i] == b'\\' && bytes.get(i + 1) == Some(&lit.quote)))
            {
                // Only runs shorter than three can have redundant quote escapes.
                let mut quotes = 0;
                let mut escapes = [0; 2];
                let mut escaped_count = 0;
                while i < bytes.len() {
                    if bytes[i] == lit.quote {
                        i += 1;
                    } else if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&lit.quote) {
                        if escaped_count < escapes.len() {
                            escapes[escaped_count] = i;
                        }
                        escaped_count += 1;
                        i += 2;
                    } else {
                        break;
                    }
                    quotes += 1;
                }
                if quotes < 3 {
                    for &index in &escapes[..escaped_count] {
                        if !(final_part && index + 2 == bytes.len()) {
                            self.report(start + index);
                        }
                    }
                }
            } else if bytes[i] == b'\\' && i + 1 < bytes.len() {
                if !is_meaningful_escape(bytes[i + 1], lit.quote) {
                    self.report(start + i);
                }
                i += 2;
            } else {
                i += 1;
            }
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) -> StructuralTraversal {
        for (lit, span) in interpolated_literals(node) {
            let mut offset = 0;
            for part in literal_parts(&lit) {
                let text = match part {
                    LiteralPart::Text(text) | LiteralPart::Interpolation(text) => text,
                };
                if matches!(part, LiteralPart::Text(_)) {
                    let trim = if offset == 0 && lit.triple {
                        multiline_lead_len(text)
                    } else {
                        0
                    };
                    self.check_text(
                        &text[trim..],
                        span.start + lit.content_offset + offset + trim,
                        lit,
                        offset + text.len() == lit.content.len(),
                    );
                }
                offset += text.len();
            }
        }
        StructuralTraversal::Continue
    }
}
}
