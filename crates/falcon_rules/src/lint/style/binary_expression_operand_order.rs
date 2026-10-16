//! Flags comparison expressions with a literal on the left-hand side.
//!
//! Writing the constant first — `if (0 == count)` — is the "Yoda condition"
//! style that reads backwards; putting the variable first (`count == 0`) matches
//! how the comparison is spoken and scanned. The check covers the equality and
//! relational operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) whenever the left
//! operand is a literal, with one exception: a `null` on the left compared
//! against another literal (as in `null == null`) is left alone.
//!
//! Checks each comparison once throughout the program, including parenthesized
//! expressions, initializers, and collections.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct BinaryExpressionOperandOrder;

impl Rule for BinaryExpressionOperandOrder {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        falcon_syntax::visitor::for_each_expr_in_program(program, |expr| {
            if let Expr::Binary {
                op,
                left,
                right,
                span,
                ..
            } = expr
            {
                let left_is_literal = is_literal_or_const(left);
                let right_is_literal = is_literal_or_const(right);
                let left_is_null = matches!(left.unparenthesized(), Expr::NullLit { .. });

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
        RulePreparation::Diagnostics(diags)
    }
}

fn is_literal_or_const(expr: &Expr) -> bool {
    matches!(
        expr.unparenthesized(),
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
