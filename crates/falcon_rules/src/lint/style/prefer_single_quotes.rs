//! Flags double-quoted string literals that could use single quotes without
//! needing an escape.
//!
//! Only use double quotes for strings whose text contains a single quote:
//!
//! ```dart
//! useStrings("should be single quote", r"should be single quote"); // flagged
//! useStrings('should be single quote', "it's fine");               // fine
//! ```
//!
//! Raw and multiline literals are checked too. An interpolated literal is left
//! alone when an interpolated expression contains another string literal, and
//! a literal nested inside another literal's interpolation is never flagged,
//! since swapping the quotes there would clash with the outer delimiters.
//! Each part of an adjacent-string concatenation is checked on its own.
//!
//! Adopted from the official `prefer_single_quotes` lint.

use crate::lint::string_lit::{LiteralPart, decoded_text, literal_parts};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferSingleQuotes;

impl Rule for PreferSingleQuotes {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(QuoteVisitor::new(
            ctx,
            QuotePreference {
                rule: "prefer-single-quotes",
                message: "Unnecessary use of double quotes.",
                preferred: b'\'',
            },
        )))
    }
}

/// The quote style a rule enforces; shared with `prefer-double-quotes`.
pub(crate) struct QuotePreference {
    pub(crate) rule: &'static str,
    pub(crate) message: &'static str,
    pub(crate) preferred: u8,
}

pub(crate) struct QuoteVisitor<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    preference: QuotePreference,
    /// How many interpolated expressions enclose the node being visited.
    interpolation_depth: usize,
}

impl<'a> QuoteVisitor<'a> {
    pub(crate) fn new(ctx: &'a AnalyzeContext<'a>, preference: QuotePreference) -> Self {
        QuoteVisitor {
            diags: Vec::new(),
            ctx,
            preference,
            interpolation_depth: 0,
        }
    }
}

structural_visitor! {
impl StructuralRuleVisitor for QuoteVisitor<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_string_interpolation(&mut self, _node: &StringInterpolation) -> StructuralTraversal {
        self.interpolation_depth += 1;
        StructuralTraversal::Continue
    }

    fn exit_string_interpolation(&mut self, _node: &StringInterpolation) {
        self.interpolation_depth -= 1;
    }

    fn visit_string_lit(&mut self, node: &StringLitNode) -> StructuralTraversal {
        if self.interpolation_depth > 0 {
            return StructuralTraversal::Continue;
        }
        let preferred = self.preference.preferred;
        let preferred_char = char::from(preferred);
        for (lit, span) in node.segments() {
            if !lit.terminated || lit.quote == preferred {
                continue;
            }
            let parts = literal_parts(&lit);
            if decoded_text(&lit, &parts).contains(preferred_char) {
                continue;
            }
            // Bail out on "strings ${x ? 'containing' : 'other'} strings".
            if parts.iter().any(|part| {
                matches!(part, LiteralPart::Interpolation(source) if source.contains(['\'', '"']))
            }) {
                continue;
            }
            self.diags.push(self.ctx.diagnostic(
                self.preference.rule,
                self.preference.message,
                span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
