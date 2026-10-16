//! Flags `.length` comparisons equivalent to a `.isNotEmpty` check.
//!
//! Collections and strings expose an `isNotEmpty` getter that reads as plain
//! intent and, for lazy iterables, can avoid computing the full `length`.
//! Comparing `length` against a constant makes the reader infer the meaning, so
//! `list.isNotEmpty` should replace forms such as `length != 0`, `length > 0`,
//! and `length >= 1`, along with their operand-swapped mirrors (`0 != length`,
//! `0 < length`, `1 <= length`). Adopted from package:lints
//! `prefer_is_not_empty`.
//!
//! Only a plain `.length` property access matches against the relevant integer
//! literal. A null-aware `receiver?.length` is deliberately left alone: that
//! comparison also has to account for a `null` receiver, so `isNotEmpty` is not
//! a drop-in replacement.
//!
//! Semantic analysis requires declaration identities. Once available, an
//! unknown receiver still triggers the lint. A `TypeIndex` can suppress it only
//! when the receiver is proven to have no `isNotEmpty` member and not to be an
//! `Iterable`, `String`, or `Map`; source-resolved members and subtypes take
//! precedence. Without a type index, those negative proofs are unavailable,
//! but source-derived receiver types are still inferred.

use super::collection_checks::{Emptiness, analyze_emptiness};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::Program;

#[derive(Default)]
pub struct PreferIsNotEmpty;
impl Rule for PreferIsNotEmpty {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(analyze_emptiness(ctx, Emptiness::NotEmpty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/rule_context.rs"
        ));
    }

    fn count_no_types(source: &str) -> usize {
        support::count(&PreferIsNotEmpty, source, false)
    }

    fn count_with_types(source: &str) -> usize {
        support::count(&PreferIsNotEmpty, source, true)
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        let src = "void f() { if (list.length != 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_collection_suppresses_only_with_types() {
        let src = "class Ruler { int get length => 3; } \
                   void f(Ruler r) { if (r.length != 0) return; }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no isNotEmpty → suppress");
    }

    #[test]
    fn class_implementing_iterable_still_fires() {
        let src = "class Bag implements Iterable<int> { int get length => 0; } \
                   void f(Bag b) { if (b.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn supertype_leaving_project_still_fires() {
        let src = "class Offsite extends Frobnicator { int get length => 0; } \
                   void f(Offsite o) { if (o.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn string_receiver_still_fires() {
        let src = "void f(String s) { if (s.length != 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn all_mirrored_forms_detected() {
        let src = "void f() { \
            if (a.length != 0) return; \
            if (0 != b.length) return; \
            if (c.length > 0) return; \
            if (0 < d.length) return; \
            if (e.length >= 1) return; \
            if (1 <= g.length) return; }";
        assert_eq!(count_no_types(src), 6);
    }
}
