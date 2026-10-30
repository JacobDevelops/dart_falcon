//! Avoid integer literals that JavaScript would round.
//!
//! Dart compiled to JavaScript stores every `int` as a 64-bit float, so an
//! integer literal that a double cannot represent exactly silently changes
//! value on the web:
//!
//! ```dart
//! int value = 9007199254740995; // BAD: becomes 9007199254740996 in JavaScript
//! BigInt value = BigInt.parse('9007199254740995'); // GOOD
//! ```
//!
//! Adopted from the official `avoid_js_rounded_ints` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal, parse_int,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidJsRoundedInts;

impl Rule for AvoidJsRoundedInts {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::IntLit(literal) = node
            && literal_value(literal.value()).is_some_and(rounds_in_javascript)
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Integer literal can't be represented exactly when compiled to JavaScript.",
                literal.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}

/// Mirrors the official `value.toDouble().toInt() != value`: `toInt` clamps to
/// the int64 range, so `i64::MAX` round-trips although no double equals it.
fn rounds_in_javascript(value: i64) -> bool {
    value != i64::MAX && !is_exactly_representable(value)
}

fn is_exactly_representable(value: i64) -> bool {
    let magnitude = value.unsigned_abs();
    magnitude <= (1_u64 << 53) || magnitude.trailing_zeros() >= magnitude.ilog2().saturating_sub(52)
}

/// The runtime `int` value of a literal: hexadecimal literals up to 64 bits
/// wrap into the signed range, decimal literals must already fit.
fn literal_value(raw: &str) -> Option<i64> {
    let value = parse_int(raw)?;
    let is_hex = raw.starts_with("0x") || raw.starts_with("0X");
    if let Ok(value) = i64::try_from(value) {
        Some(value)
    } else if is_hex {
        u64::try_from(value).ok().map(u64::cast_signed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{is_exactly_representable, literal_value, rounds_in_javascript};

    #[test]
    fn exactness_at_precision_and_signed_boundaries() {
        for (value, exact) in [
            (0, true),
            (1_i64 << 53, true),
            ((1_i64 << 53) + 1, false),
            ((1_i64 << 53) + 2, true),
            (i64::MIN, true),
            (i64::MIN + 1, false),
            (i64::MAX, false),
            (i64::MAX - 1023, true),
        ] {
            assert_eq!(is_exactly_representable(value), exact, "{value}");
            if let Some(negated) = value.checked_neg() {
                assert_eq!(is_exactly_representable(negated), exact, "{negated}");
            }
        }
    }

    #[test]
    fn int64_max_round_trips_like_the_official_lint() {
        assert!(!rounds_in_javascript(i64::MAX));
        assert!(rounds_in_javascript(i64::MAX - 1));
        assert!(rounds_in_javascript(-i64::MAX));
    }

    #[test]
    fn hexadecimal_literals_use_signed_runtime_values() {
        for (raw, value, exact) in [
            ("0xffffffffffffffff", -1, true),
            ("0x8000000000000000", i64::MIN, true),
            ("0x7fffffffffffffff", i64::MAX, false),
            ("0x8000000000000001", i64::MIN + 1, false),
        ] {
            assert_eq!(literal_value(raw), Some(value), "{raw}");
            assert_eq!(is_exactly_representable(value), exact, "{raw}");
        }
        assert_eq!(literal_value("9223372036854775808"), None);
        assert_eq!(literal_value("0x10000000000000000"), None);
    }
}
