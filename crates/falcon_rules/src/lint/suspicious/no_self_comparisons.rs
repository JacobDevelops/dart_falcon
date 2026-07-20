//! Flags a comparison whose two operands are identical.
//!
//! An expression like `x == x` or `count < count` compares a value with itself,
//! so `==`, `<=`, and `>=` are always true while `!=`, `<`, and `>` are always
//! false. It is almost always a typo for a different variable or a leftover from
//! a refactor, and the constant result masks the check that was intended.
//! Operands are compared by source text with whitespace removed, covering `==`,
//! `!=`, `<`, `>`, `<=`, and `>=`. Fix the operand that was meant to differ.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct NoSelfComparisons;

impl Rule for NoSelfComparisons {
    fn name(&self) -> &'static str {
        "no-self-comparisons"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    file: String,
    source: &'s str,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Binary {
            op,
            left,
            right,
            span,
        } = node
            && is_comparison(op)
        {
            let l = strip_ws(&self.source[left.span().start..left.span().end]);
            let r = strip_ws(&self.source[right.span().start..right.span().end]);
            if !l.is_empty() && l == r && is_side_effect_free(left) && is_side_effect_free(right) {
                self.diags.push(Diagnostic::new(
                    "no-self-comparisons",
                    Severity::Warning,
                    "Both operands of this comparison are identical.",
                    self.file.clone(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
        }
        walk_expr(self, node);
    }
}

fn is_comparison(op: &BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
    )
}

/// Collapse an operand's source text by removing all whitespace, so that
/// formatting differences do not hide a structurally identical comparison.
fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Two textually identical operands are only a self-comparison when evaluating
/// them twice yields the same value. A call/constructor may return a different
/// value each time (`list.removeLast() == list.removeLast()`), so any operand
/// reaching an invocation is not treated as identical.
fn is_side_effect_free(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(_)
        | Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. }
        | Expr::SymbolLit { .. }
        | Expr::This { .. }
        | Expr::Super { .. } => true,
        Expr::StringLit(s) => s
            .interpolations
            .iter()
            .all(|i| is_side_effect_free(&i.expr)),
        Expr::Field { object, .. } => is_side_effect_free(object),
        Expr::Index { object, index, .. } => {
            is_side_effect_free(object) && is_side_effect_free(index)
        }
        Expr::Unary { operand, .. } | Expr::NullAssert { operand, .. } => {
            is_side_effect_free(operand)
        }
        Expr::Is { expr, .. } | Expr::As { expr, .. } => is_side_effect_free(expr),
        Expr::Binary { left, right, .. } => is_side_effect_free(left) && is_side_effect_free(right),
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            is_side_effect_free(condition)
                && is_side_effect_free(then_expr)
                && is_side_effect_free(else_expr)
        }
        _ => false,
    }
}
