//! Flags a cascade after an unparenthesized null-coalescing expression.
//!
//! In `a ?? b..method()`, the cascade applies to the result of `a ?? b`, not
//! just `b`. Parenthesize the receiver as `(a ?? b)..method()` or the fallback
//! as `a ?? (b..method())` to make that choice explicit. The receiver may also
//! be a ternary whose trailing else expression ends in `??`. Explicit receiver
//! parentheses and parentheses around the fallback are respected, and `??`
//! nested in other receiver subexpressions is not flagged.
//! Both ordinary and null-aware cascades are checked without type inference.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct CascadeAfterNullCoalescing;

impl Rule for CascadeAfterNullCoalescing {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Cascade(cascade) = node
            && ends_in_null_coalescing(cascade.object())
        {
            self.diags.push(self.ctx.diagnostic(
                "Cascade after `??` applies to the whole receiver; parenthesize the receiver or fallback.",
                cascade.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}

fn ends_in_null_coalescing(mut receiver: Expr<'_>) -> bool {
    while let Expr::Conditional(conditional) = receiver {
        receiver = conditional.else_expr();
    }
    matches!(receiver, Expr::Binary(binary) if binary.op() == BinaryOp::NullCoalesce)
}
