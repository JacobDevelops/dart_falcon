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
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::ast::*;
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
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(lib) = &program.library_directive else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let (Some(first), Some(last)) = (lib.name.first(), lib.name.last()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let name = lib
            .name
            .iter()
            .map(|segment| segment.name.as_str())
            .collect::<Vec<_>>()
            .join(".");
        if LOWER_CASE_WITH_DOTS.is_match(&name) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Diagnostics(vec![ctx.diagnostic(
            "library-names",
            format!("The library name '{name}' isn't a lower_case_with_underscores identifier."),
            DiagSpan {
                start: first.span.start,
                end: last.span.end,
            },
        )])
    }
}
