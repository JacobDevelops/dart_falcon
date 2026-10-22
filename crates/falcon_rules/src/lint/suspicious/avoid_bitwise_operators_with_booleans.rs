//! Flags `&` and `|` applied to two non-nullable boolean operands.
//!
//! Prefer `&&` and `||` to express logical intent and short-circuit evaluation.
//! Both operands must resolve to core `bool`; integer, dynamic, nullable, and
//! unresolved operands are left alone. Exclusive-or and compound assignments
//! are not replacements for logical conjunction or disjunction.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidBitwiseOperatorsWithBooleans;

impl Rule for AvoidBitwiseOperatorsWithBooleans {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        let replacement = match binary.op() {
            BinaryOp::BitAnd => "&&",
            BinaryOp::BitOr => "||",
            _ => return,
        };
        let boolean = |operand| {
            let ty = state.infer(operand);
            ty.interface("dart:core", "bool") && !ty.nullable()
        };
        if boolean(binary.left()) && boolean(binary.right()) {
            self.diagnostics.push(self.ctx.diagnostic(format!("Use `{replacement}` instead of a bitwise operator on booleans."), binary.span()));
        }
    }
}
