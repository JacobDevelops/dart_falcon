//! Prefer direct comparisons and De Morgan forms in control-flow conditions.
//! Unlike avoid-inverted-boolean-expressions, this does not diagnose `!!value`.

use super::avoid_assignments_as_conditions::is_in_condition;
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
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Unary(unary) = node
            && unary.op() == UnaryOp::Bang
            && is_in_condition(unary.syntax())
            && let Expr::Binary(binary) = unary.operand().unparenthesized()
            && matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq | BinaryOp::Lt
                | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::And | BinaryOp::Or)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-inverted-boolean-checks",
                "Express this check directly instead of negating a comparison or logical expression.",
                unary.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
