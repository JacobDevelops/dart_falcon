//! Don't compare boolean values to boolean literals.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{BinaryOp, Expr, Program};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct NoBooleanLiteralCompare;

impl Rule for NoBooleanLiteralCompare {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if !matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) {
            return;
        }
        let operand = match (
            binary.left().unparenthesized(),
            binary.right().unparenthesized(),
        ) {
            (Expr::BoolLit(_), operand) | (operand, Expr::BoolLit(_)) => operand,
            _ => return,
        };
        let ty = state.infer(operand);
        if ty.interface("dart:core", "bool") && !ty.nullable() {
            self.diags.push(self.ctx.diagnostic("Avoid comparing boolean values to boolean literals.", binary.span()));
        }
    }
}
