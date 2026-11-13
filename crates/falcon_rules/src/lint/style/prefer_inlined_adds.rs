//! Flags list-literal cascades whose added elements could be written inline.
//!
//! Both `[a]..add(b)` and `[a]..addAll([b, c])` can put the added elements
//! directly in the original literal. Calls require one positional argument
//! and no named arguments. Nonliteral receivers and `addAll` arguments are
//! left alone; `prefer-spread-collections` handles nonliteral arguments.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferInlinedAdds;

impl Rule for PreferInlinedAdds {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
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
        if let Expr::Cascade(cascade) = node.unparenthesized()
            && matches!(cascade.object().unparenthesized(), Expr::List(_))
        {
            for section in cascade.sections() {
                for op in section.ops() {
                    if let CascadeOp::Call(call) = op
                        && call.args().positional().len() == 1
                        && call.args().named().is_empty()
                        && (call.name().name() == "add"
                            || (call.name().name() == "addAll"
                                && matches!(call.args().positional().next().unwrap().unparenthesized(), Expr::List(_))))
                    {
                        self.diags.push(self.ctx.diagnostic(
                            "Inline the added elements into the list literal.",
                            call.name().span(),
                        ));
                        break;
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
