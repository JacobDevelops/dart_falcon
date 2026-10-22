//! Flags single-quoted string literals that could use double quotes without
//! needing an escape.
//!
//! Prefer double quotes unless the string's text contains a double quote:
//!
//! ```dart
//! useStrings('should be double quote', r'should be double quote'); // flagged
//! useStrings("should be double quote", 'contains " quote');        // fine
//! ```
//!
//! Raw and multiline literals are checked too. An interpolated literal is left
//! alone when an interpolated expression contains another string literal, and
//! a literal nested inside another literal's interpolation is never flagged,
//! since swapping the quotes there would clash with the outer delimiters.
//! Each part of an adjacent-string concatenation is checked on its own. This
//! rule is the opposite of `prefer-single-quotes`; enable only one.
//!
//! Adopted from the official `prefer_double_quotes` lint.

use crate::lint::support::quote_style::{QuotePreference, QuoteVisitor};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferDoubleQuotes;

impl Rule for PreferDoubleQuotes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(QuoteVisitor::new(
            ctx,
            QuotePreference {
                message: "Unnecessary use of single quotes.",
                preferred: b'"',
            },
        )))
    }
}
