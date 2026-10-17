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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct BinaryExpressionOperandOrder;

impl Rule for BinaryExpressionOperandOrder {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expr: Expr<'a>) -> StructuralTraversal {
            if let Expr::Binary(binary) = expr {
                let op = binary.op();
                let left = binary.left();
                let right = binary.right();
                let span = binary.span();
                let left_is_literal = is_literal_or_const(left);
                let right_is_literal = is_literal_or_const(right);
                let left_is_null = matches!(left.unparenthesized(), Expr::NullLit(_));

                if should_check_operator(op)
                    && left_is_literal
                    && !(left_is_null && right_is_literal)
                {
                    self.diags.push(self.ctx.diagnostic(
                "binary-expression-operand-order",
                "Avoid placing literal or constant operand on the left side of a binary expression",
                span,
            ));
                }
            }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_literal_or_const(expr: Expr<'_>) -> bool {
    matches!(
        expr.unparenthesized(),
        Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
    )
}

fn should_check_operator(op: BinaryOp) -> bool {
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
