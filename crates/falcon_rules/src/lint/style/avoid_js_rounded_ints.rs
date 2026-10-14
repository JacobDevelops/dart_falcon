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

use falcon_analyze::{AnalyzeContext, Rule, parse_int};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

#[derive(Default)]
pub struct AvoidJsRoundedInts;

impl Rule for AvoidJsRoundedInts {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        walk_program(&mut collector, program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::IntLit { value, span } = node
            && literal_value(value).is_some_and(|value| (value as f64) as i64 != value)
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-js-rounded-ints",
                "Integer literal can't be represented exactly when compiled to JavaScript.",
                *span,
            ));
        }
        walk_expr(self, node);
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
