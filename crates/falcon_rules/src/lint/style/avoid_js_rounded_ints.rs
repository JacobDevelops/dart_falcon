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
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
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
            && literal_value(literal.value()).is_some_and(|value| (value as f64) as i64 != value)
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

/// The runtime `int` value of a literal: hexadecimal literals up to 64 bits
/// wrap into the signed range, decimal literals must already fit.
fn literal_value(raw: &str) -> Option<i64> {
    let value = parse_int(raw)?;
    let is_hex = raw.starts_with("0x") || raw.starts_with("0X");
    if let Ok(value) = i64::try_from(value) {
        Some(value)
    } else if is_hex {
        u64::try_from(value).ok().map(|value| value as i64)
    } else {
        None
    }
}
