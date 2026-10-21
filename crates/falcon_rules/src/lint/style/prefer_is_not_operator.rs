//! Flags a negated type test `!(x is T)` that should use the `is!` operator.
//!
//! Dart provides the `is!` operator precisely so negated type checks read
//! directly as `x is! T` instead of wrapping the test in parentheses and negating
//! it, which is easier to misread and to accidentally mis-parenthesize. The rule
//! fires on a logical-not applied to a non-negated `is` expression; a test that
//! already uses `is!` produces no negation to flag.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferIsNotOperator;

impl Rule for PreferIsNotOperator {
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
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Unary(unary) = node.unparenthesized()
            && unary.op() == UnaryOp::Bang
            && matches!(unary.operand().unparenthesized(), Expr::Is(test) if !test.negated())
        {
            self.diags.push(self.ctx.diagnostic(
                "Prefer using the 'is!' operator.",
                unary.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
