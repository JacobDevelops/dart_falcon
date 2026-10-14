//! Don't compare boolean values to boolean literals.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::{BinaryOp, Expr};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct NoBooleanLiteralCompare;

impl Rule for NoBooleanLiteralCompare {
    fn name(&self) -> &'static str {
        "no-boolean-literal-compare"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op: BinaryOp::EqEq | BinaryOp::NotEq,
            left,
            right,
            span,
        } = node
        else {
            return;
        };
        let operand = match (left.as_ref(), right.as_ref()) {
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
