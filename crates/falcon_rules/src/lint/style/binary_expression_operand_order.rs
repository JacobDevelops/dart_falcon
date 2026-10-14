//! Flags comparison expressions with a literal on the left-hand side.
//!
//! Writing the constant first — `if (0 == count)` — is the "Yoda condition"
//! style that reads backwards; putting the variable first (`count == 0`) matches
//! how the comparison is spoken and scanned. The check covers the equality and
//! relational operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) whenever the left
//! operand is a literal, with one exception: a `null` on the left compared
//! against another literal (as in `null == null`) is left alone.
//!
//! Checks comparisons throughout the program, including initializers and collections.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct BinaryExpressionOperandOrder;

impl Rule for BinaryExpressionOperandOrder {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        super::traversal::for_each_expr(program, |expr| {
            if let Expr::Binary {
                op,
                left,
                right,
                span,
            } = expr
            {
                let left_is_literal = is_literal_or_const(left);
                let right_is_literal = is_literal_or_const(right);
                let left_is_null = matches!(left.as_ref(), Expr::NullLit { .. });

                if should_check_operator(op)
                    && left_is_literal
                    && !(left_is_null && right_is_literal)
                {
                    diags.push(ctx.diagnostic(
                        "binary-expression-operand-order",
                        "Avoid placing literal or constant operand on the left side of a binary expression",
                        *span,
                    ));
                }
            }
        });
        diags
    }
}

fn is_literal_or_const(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::StringLit(_)
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
    )
}

fn should_check_operator(op: &BinaryOp) -> bool {
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
