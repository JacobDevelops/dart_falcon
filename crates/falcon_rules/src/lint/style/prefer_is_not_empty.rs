//! Flags negated `isEmpty` checks in favor of `isNotEmpty`.
//!
//! `!items.isEmpty` and `!(items.isEmpty)` are more directly expressed as
//! `items.isNotEmpty`. Null-aware accesses are left alone. Length comparisons
//! belong to `prefer-is-empty`, irrespective of the direction of comparison.
//! Adopted from package:lints `prefer_is_not_empty`.
//!
//! Unknown receivers still trigger the lint. A type index suppresses a report
//! when the receiver is proven not to provide `isNotEmpty` and is not an
//! `Iterable`, `String`, or `Map`.

use crate::lint::support::collection_checks::{Emptiness, analyze_emptiness};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferIsNotEmpty;
impl Rule for PreferIsNotEmpty {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(analyze_emptiness(ctx, Emptiness::NegatedIsEmpty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::test_support::count;

    fn count_no_types(source: &str) -> usize {
        count("prefer-is-not-empty", &PreferIsNotEmpty, source, false)
    }

    fn count_with_types(source: &str) -> usize {
        count("prefer-is-not-empty", &PreferIsNotEmpty, source, true)
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        let src = "void f() { if (!list.isEmpty) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_collection_suppresses_only_with_types() {
        let src = "class Ruler { bool get isEmpty => true; } \
                   void f(Ruler r) { if (!r.isEmpty) return; }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no isNotEmpty → suppress");
    }

    #[test]
    fn class_implementing_iterable_still_fires() {
        let src = "class Bag implements Iterable<int> { int get length => 0; } \
                   void f(Bag b) { if (!b.isEmpty) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn supertype_leaving_project_still_fires() {
        let src = "class Offsite extends Frobnicator { bool get isEmpty => true; } \
                   void f(Offsite o) { if (!o.isEmpty) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn string_receiver_still_fires() {
        let src = "void f(String s) { if (!s.isEmpty) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn parenthesized_checks_detected() {
        let src = "void f() { \
            if (!a.isEmpty) return; \
            if (!(b.isEmpty)) return; \
            if (!((c.isEmpty))) return; }";
        assert_eq!(count_no_types(src), 3);
    }
}
