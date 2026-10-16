//! Don't compare boolean values to boolean literals.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::{BinaryOp, Expr};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct NoBooleanLiteralCompare;

impl Rule for NoBooleanLiteralCompare {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op: BinaryOp::EqEq | BinaryOp::NotEq,
            left,
            right,
            span,
            ..
        } = node
        else {
            return;
        };
        let operand = match (left.unparenthesized(), right.unparenthesized()) {
            (Expr::BoolLit { .. }, operand) | (operand, Expr::BoolLit { .. }) => operand,
            _ => return,
        };
        let ty = state.infer(operand);
        if ty.interface("dart:core", "bool") && !ty.nullable() {
            self.diags.push(self.ctx.diagnostic(
                "no-boolean-literal-compare",
                "Avoid comparing boolean values to boolean literals",
                *span,
            ));
        }
    }
}
