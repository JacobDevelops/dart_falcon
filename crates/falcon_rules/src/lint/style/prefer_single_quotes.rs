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


use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation,

};

use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferSingleQuotes;

impl Rule for PreferSingleQuotes {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(QuoteVisitor::new(
            ctx,
            QuotePreference {
                message: "Unnecessary use of double quotes.",
                preferred: b'\'',
            },
        )))
    }
}

use crate::lint::support::quote_style::{QuotePreference, QuoteVisitor};
