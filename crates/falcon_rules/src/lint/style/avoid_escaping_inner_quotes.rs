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
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidEscapingInnerQuotes;

impl Rule for AvoidEscapingInnerQuotes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) -> StructuralTraversal {
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
                    format!("Unnecessary escape of '{own}'; change the outer quotes to '{other}'."),
                    span,
                ));
            }
        }
        StructuralTraversal::Continue
    }
}
}
