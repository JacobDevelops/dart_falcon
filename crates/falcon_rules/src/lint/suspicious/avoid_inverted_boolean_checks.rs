//! Prefer direct equality comparisons and De Morgan forms in control-flow conditions.
//! Unlike avoid-inverted-boolean-expressions, this does not diagnose `!!value`.
//! Relational operators are excluded because NaN and custom operators lack reliable inverses.

use crate::lint::support::condition_position::is_in_condition;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidInvertedBooleanChecks;

impl Rule for AvoidInvertedBooleanChecks {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Unary(unary) = node
            && unary.op() == UnaryOp::Bang
            && is_in_condition(unary.syntax())
            && let Expr::Binary(binary) = unary.operand().unparenthesized()
            && matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or)
        {
            self.diags.push(self.ctx.diagnostic(
                "Express this check directly instead of negating a comparison or logical expression.",
                unary.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
