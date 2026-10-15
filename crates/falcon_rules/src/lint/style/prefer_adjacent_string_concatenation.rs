//! Flags two string literals joined with the `+` operator.
//!
//! Dart concatenates adjacent string literals at compile time, so `'a' 'b'` is
//! equivalent to `'a' + 'b'` but without the runtime operator or the visual
//! noise. The rule fires only when both operands of an `Add` expression are
//! string literals; a literal added to any other expression is left alone, since
//! adjacency cannot express that. Prefer writing the pieces side by side, which
//! also reads naturally when splitting a long string across lines.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_expr_in_program;

#[derive(Default)]
pub struct PreferAdjacentStringConcatenation;

impl Rule for PreferAdjacentStringConcatenation {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_expr_in_program(program, |node| {
            if let Expr::Binary {
                op: BinaryOp::Add,
                left,
                right,
                span,
                ..
            } = node.unparenthesized()
                && matches!(left.unparenthesized(), Expr::StringLit(_))
                && matches!(right.unparenthesized(), Expr::StringLit(_))
            {
                diags.push(ctx.diagnostic(
                    "prefer-adjacent-string-concatenation",
                    "Use adjacent string literals instead of the `+` operator.",
                    *span,
                ));
            }
        });
        diags
    }
}
