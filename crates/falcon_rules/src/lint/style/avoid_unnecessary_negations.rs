//! Simplifies negated bool literals and equality comparisons with a logical
//! negation on both sides. Stacked negations belong to
//! `avoid-inverted-boolean-expressions`; arithmetic, bitwise and relational
//! operators are deliberately excluded.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryNegations;

impl Rule for AvoidUnnecessaryNegations {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        let redundant = match node {
            Expr::Unary(unary) if unary.op() == UnaryOp::Bang => {
                let mut parent = unary.syntax().parent();
                while parent.is_some_and(|node| matches!(Expr::cast(node), Some(Expr::Paren(_)))) {
                    parent = parent.and_then(|node| node.parent());
                }
                if parent.is_some_and(|node| matches!(Expr::cast(node),
                    Some(Expr::Unary(outer)) if outer.op() == UnaryOp::Bang))
                {
                    return StructuralTraversal::Continue;
                }
                let operand = unary.operand().unparenthesized();
                matches!(operand, Expr::BoolLit(_))
            }
            Expr::Binary(binary) => {
                matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq)
                    && is_negation(binary.left())
                    && is_negation(binary.right())
            }
            _ => false,
        };
        if redundant {
            self.diagnostics.push(self.ctx.diagnostic(
                "Simplify these unnecessary logical negations.",
                node.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

fn is_negation(expression: Expr<'_>) -> bool {
    matches!(expression.unparenthesized(), Expr::Unary(unary) if unary.op() == UnaryOp::Bang)
}
