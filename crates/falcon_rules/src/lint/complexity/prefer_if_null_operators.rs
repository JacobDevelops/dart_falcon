//! Flags `x == null ? y : x` (and the inverted `x != null ? x : y`).
//!
//! A ternary that returns its own operand on the non-null branch is exactly the
//! `??` if-null operator: `x ?? y`. The operator is shorter and evaluates `x`
//! only once. The rule takes the operand of the `== null`/`!= null` check and
//! compares it (ignoring whitespace) against the branch that falls through to
//! it, firing when the two are textually equal.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferIfNullOperators;

impl Rule for PreferIfNullOperators {
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

use crate::lint::expressions::{equivalent, null_check};

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Conditional(conditional) = node
            && let Some((operand, is_eq)) = null_check(conditional.condition())
        {
            // `x == null ? y : x` keeps `x` in the else branch;
            // `x != null ? x : y` keeps `x` in the then branch.
            let fallthrough = if is_eq { conditional.else_expr() } else { conditional.then_expr() };
            if equivalent(operand, fallthrough, self.ctx) {
                self.diags.push(self.ctx.diagnostic(
                    "Prefer using the if-null operator `??`.",
                    conditional.span(),
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
