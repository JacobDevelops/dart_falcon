//! Flags a library name that is not `lowercase_with_underscores`.
//!
//! Library names are global identifiers; Effective Dart spells them in lower
//! case with underscores, with optional dot-separated segments:
//!
//! ```dart
//! library peg_parser.source_scanner;  // fine
//! library PegParser.SourceScanner;    // flagged
//! ```
//!
//! The first segment may start with one underscore or a digit; later segments
//! must start with a lower-case letter.
//!
//! Adopted from the official `library_names` lint.

use std::sync::LazyLock;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;
use regex::Regex;

/// The analyzer's `isLowerCaseUnderScoreWithDots` (ASCII digits only).
static LOWER_CASE_WITH_DOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^_?[_a-z0-9]*(?:\.[a-z][_a-z0-9]*)*$").expect("valid library name regex")
});

#[derive(Default)]
pub struct LibraryNames;

impl Rule for LibraryNames {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(lib) = program.library_directive() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let mut segments = lib.name();
        let Some(first) = segments.next() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let mut name = String::with_capacity(lib.span().end - first.span().start);
        name.push_str(first.name());
        let mut span = first.span();
        for segment in segments {
            name.push('.');
            name.push_str(segment.name());
            span.end = segment.span().end;
        }
        if LOWER_CASE_WITH_DOTS.is_match(&name) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Diagnostics(vec![ctx.diagnostic(
            format!("The library name `{name}` isn't a `lower_case_with_underscores` identifier."),
            span,
        )])
    }
}
