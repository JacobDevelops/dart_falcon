//! Flags loop conditions with more logical operands than the configured threshold.
//!
//! Checks `while`, `do`/`while`, and C-style `for` conditions. Each operand
//! joined by `&&` or `||` counts as one; comparisons and calls are opaque.
//! For-in iterables and for-loop initializers and updates are not conditions.
//!
//! ## Options
//!
//! `threshold` (integer, default: 1) — maximum number of logical operands.

use super::avoid_complex_conditions::logical_operands;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidComplexLoopConditions;

impl Rule for AvoidComplexLoopConditions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(LoopConditions {
            ctx,
            threshold: crate::lint::options::usize(
                crate::lint::options::for_rule(ctx, "avoid-complex-loop-conditions"),
                "threshold",
            )
            .unwrap_or(1),
            diags: Vec::new(),
        }))
    }
}

struct LoopConditions<'a> {
    ctx: &'a AnalyzeContext<'a>,
    threshold: usize,
    diags: Vec<Diagnostic>,
}

impl LoopConditions<'_> {
    fn check(&mut self, condition: Expr<'_>) {
        let count = logical_operands(condition);
        if count > self.threshold {
            self.diags.push(self.ctx.diagnostic("avoid-complex-loop-conditions", format!("Loop condition has {count} logical operands (max {}). Extract a named condition.", self.threshold), condition.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for LoopConditions<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        match node {
            Stmt::While(node) => self.check(node.condition()),
            Stmt::DoWhile(node) => self.check(node.condition()),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_for_header(&mut self, node: ForHeader<'a>) -> StructuralTraversal {
        if let Some(condition) = node.condition() { self.check(condition); }
        StructuralTraversal::Continue
    }
}
}
