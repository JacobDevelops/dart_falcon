//! Flags an `addAll` cascade on a list literal that a spread would express.
//!
//! `[a, b]..addAll(items)` is more directly written `[a, b, ...items]`.
//! Calls require one positional argument and no named arguments. List-literal
//! arguments belong to `prefer-inlined-adds`; other receivers are left alone.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferSpreadCollections;

impl Rule for PreferSpreadCollections {
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
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Cascade(cascade) = node.unparenthesized()
            && matches!(cascade.object().unparenthesized(), Expr::List(_))
        {
            for section in cascade.sections() {
                for op in section.ops() {
                    if let CascadeOp::Call(call) = op
                        && call.name().name() == "addAll"
                        && call.args().positional().len() == 1
                        && call.args().named().is_empty()
                        && !matches!(call.args().positional().next().unwrap().unparenthesized(), Expr::List(_))
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "Use the spread operator `...` instead of `addAll`.",
                            call.name().span(),
                        ));
                        break;
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
