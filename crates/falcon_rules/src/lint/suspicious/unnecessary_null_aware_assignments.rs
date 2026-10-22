//! Flags `x ??= null`, which can never have an effect.
//!
//! The `??=` operator assigns only when the target is currently null, so
//! assigning `null` is a guaranteed no-op: a null target stays null, and
//! otherwise nothing happens. Such a statement is dead code that usually points
//! to a mistaken right-hand side — the author likely meant a real default value.
//! Remove the statement, or supply the value that was intended.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryNullAwareAssignments;

impl Rule for UnnecessaryNullAwareAssignments {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Assign(assign) = node
            && assign.op() == AssignOp::NullCoalesceEq
            && matches!(assign.value().unparenthesized(), Expr::NullLit(_))
        {
            self.diags.push(self.ctx.diagnostic(
                "Remove this unnecessary null-aware assignment to null.",
                assign.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
