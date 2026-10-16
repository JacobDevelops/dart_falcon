//! Flags `show`/`hide` combinators whose names are not sorted alphabetically.
//!
//! Sorted combinator lists are easier to scan and produce smaller diffs when a
//! name is added. Each `show` or `hide` clause on an import or export is checked
//! on its own; the clause is reported when its names are out of order:
//!
//! ```dart
//! import 'a.dart' show B, A;        // flagged
//! import 'a.dart' show A, B hide D, C; // `hide D, C` flagged
//! import 'a.dart' show A, B;
//! ```
//!
//! Adopted from the official `combinators_ordering` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct CombinatorsOrdering;

impl Rule for CombinatorsOrdering {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let combinators = program
            .imports
            .iter()
            .flat_map(|import| &import.combinators)
            .chain(
                program
                    .exports
                    .iter()
                    .flat_map(|export| &export.combinators),
            );
        RulePreparation::Diagnostics(combinators
            .filter_map(|combinator| {
                let (ImportCombinator::Show(names, span) | ImportCombinator::Hide(names, span)) =
                    combinator;
                let sorted = names
                    .windows(2)
                    .all(|pair| dart_compare(&pair[0].name, &pair[1].name).is_le());
                (!sorted).then(|| {
                    ctx.diagnostic(
                        "combinators-ordering",
                        "Sort combinator names alphabetically.",
                        *span,
                    )
                })
            })
            .collect())
    }
}

/// Dart's `String.compareTo`: lexicographic over UTF-16 code units.
fn dart_compare(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}
