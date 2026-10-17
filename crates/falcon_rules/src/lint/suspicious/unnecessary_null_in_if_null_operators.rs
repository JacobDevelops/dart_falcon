//! Flags `x ?? null` and `null ?? x`, where the `null` operand is redundant.
//!
//! The `??` operator falls back to its right operand only when the left is null,
//! so `x ?? null` always evaluates to `x` and `null ?? x` always evaluates to
//! `x`. The literal `null` contributes nothing and typically marks an unfinished
//! edit or a misunderstanding of the operator. Delete the `null` operand, and the
//! `??` along with it.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UnnecessaryNullInIfNullOperators;

impl Rule for UnnecessaryNullInIfNullOperators {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Binary {
            op: BinaryOp::NullCoalesce,
            left,
            right,
            span,
            ..
        } = node
            && (matches!(left.unparenthesized(), Expr::NullLit { .. })
                || matches!(right.unparenthesized(), Expr::NullLit { .. }))
        {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-null-in-if-null-operators",
                "Unnecessary null in an if-null ('??') operator.",
                *span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
