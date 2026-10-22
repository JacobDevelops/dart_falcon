//! Flags two string literals joined with the `+` operator.
//!
//! Dart concatenates adjacent string literals at compile time, so `'a' 'b'` is
//! equivalent to `'a' + 'b'` but without the runtime operator or the visual
//! noise. The rule fires only when both operands of an `Add` expression are
//! string literals; a literal added to any other expression is left alone, since
//! adjacency cannot express that. Prefer writing the pieces side by side, which
//! also reads naturally when splitting a long string across lines.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferAdjacentStringConcatenation;

impl Rule for PreferAdjacentStringConcatenation {
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Binary(binary) = node.unparenthesized()
            && binary.op() == BinaryOp::Add
            && matches!(binary.left().unparenthesized(), Expr::StringLit(_))
            && matches!(binary.right().unparenthesized(), Expr::StringLit(_))
        {
            self.diags.push(self.ctx.diagnostic(
                "Use adjacent string literals instead of the `+` operator.",
                binary.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
