//! Flags `.length` comparisons equivalent to a `.isEmpty` check.
//!
//! Collections and strings expose an `isEmpty` getter that states intent
//! directly and, for lazy iterables, can be cheaper than computing the full
//! `length`. Comparing `length` against a constant forces the reader to decode
//! the boundary, so `list.isEmpty` should replace forms such as `length == 0`,
//! `length < 1`, and `length <= 0`, together with their operand-swapped mirrors
//! (`0 == length`, `1 > length`, `0 >= length`). Adopted from package:lints
//! `prefer_is_empty`.
//!
//! Only a plain `.length` property access matches against the relevant integer
//! literal. A null-aware `receiver?.length` is deliberately left alone: that
//! comparison also has to account for a `null` receiver, so `isEmpty` is not a
//! drop-in replacement.
//!
//! Semantic analysis requires declaration identities. Once available, an
//! unknown receiver still triggers the lint. A `TypeIndex` can suppress it only
//! when the receiver is proven to have no `isEmpty` member and not to be an
//! `Iterable`, `String`, or `Map`; source-resolved members and subtypes take
//! precedence. Without a type index, those negative proofs are unavailable,
//! but source-derived receiver types are still inferred.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use super::collection_checks::{Emptiness, analyze_emptiness};

#[derive(Default)]
pub struct PreferIsEmpty;
impl Rule for PreferIsEmpty {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(analyze_emptiness(ctx, Emptiness::Empty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::test_support::count;

    fn count_no_types(source: &str) -> usize {
        count("prefer-is-empty", &PreferIsEmpty, source, false)
    }

    fn count_with_types(source: &str) -> usize {
        count("prefer-is-empty", &PreferIsEmpty, source, true)
    }

    #[test]
    fn unknown_receiver_fires_with_and_without_types() {
        // Bare identifier — Unknown type — must fire either way (the baseline).
        let src = "void f() { if (list.length == 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn proven_non_collection_suppresses_only_with_types() {
        // `Ruler` has only `length`; not a collection/string → Absent isEmpty.
        let src = "class Ruler { int get length => 3; } \
                   void f(Ruler r) { if (r.length == 0) return; }";
        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(count_with_types(src), 0, "proven no isEmpty → suppress");
    }

    #[test]
    fn class_implementing_iterable_still_fires() {
        // Bag inherits `isEmpty` from Iterable → member Found, not Absent.
        let src = "class Bag implements Iterable<int> { int get length => 0; } \
                   void f(Bag b) { if (b.length == 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn supertype_leaving_project_still_fires() {
        // Unknown ancestor → chain not fully resolved → Unknown, never suppress.
        let src = "class Offsite extends Frobnicator { int get length => 0; } \
                   void f(Offsite o) { if (o.length == 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn static_method_call_receiver_still_fires() {
        // `Config.load()` is a *static method* returning a Map, not a `Config`
        // instance. Its receiver type must stay Unknown, so the diagnostic keeps
        // firing — mis-typing it as `Config` (which lacks isEmpty) would suppress.
        let src = "class Config { static Map<String, String> load() => {}; } \
                   void f() { if (Config.load().length == 0) return; }";
        assert_eq!(count_no_types(src), 1);
        assert_eq!(
            count_with_types(src),
            1,
            "static-method return is Unknown → fire"
        );
    }

    #[test]
    fn named_constructor_call_receiver_suppresses() {
        // `Box.zero()` is a named constructor → receiver type is `Box`. `Box` has
        // only `length`, no isEmpty, and is no collection → proven → suppress.
        let src = "class Box { Box.zero(); int get length => 0; } \
                   void f() { if (Box.zero().length == 0) return; }";

        assert_eq!(count_no_types(src), 1, "no type index → fire (baseline)");
        assert_eq!(
            count_with_types(src),
            0,
            "named ctor → Box, proven no isEmpty → suppress"
        );
    }

    #[test]
    fn string_receiver_still_fires() {
        // String has isEmpty; suggesting isEmpty is correct → keep firing.
        let src = "void f(String s) { if (s.length == 0) return; }";
        assert_eq!(count_with_types(src), 1);
    }

    #[test]
    fn all_mirrored_forms_detected() {
        let src = "void f() { \
            if (a.length == 0) return; \
            if (0 == b.length) return; \
            if (c.length < 1) return; \
            if (d.length <= 0) return; \
            if (1 > e.length) return; \
            if (0 >= g.length) return; }";
        assert_eq!(count_no_types(src), 6);
    }
}
