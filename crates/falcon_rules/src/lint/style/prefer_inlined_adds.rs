//! Flags an `add` cascade on a list literal whose element could be written inline.
//!
//! Building a list by literal-then-cascade (`[a]..add(b)`) is more verbose than
//! placing the element directly in the literal (`[a, b]`), and the inline form
//! reads as a single collection rather than a construction followed by mutation.
//! The rule matches each `..add(...)` section cascaded onto a list literal that
//! takes exactly one positional argument and no named arguments; multi-argument
//! or named calls, and cascades on non-list receivers, are ignored.

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
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
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
                // Stop after one match because operations in a section share its span.
                for op in section.ops() {
                    if let CascadeOp::Call(call) = op
                        && call.name().name() == "add"
                        && call.args().positional().len() == 1
                        && call.args().named().is_empty()
                    {
                        self.diags.push(self.ctx.diagnostic("Inline the added element into the collection literal instead of using `add`.", section.span()));
                        break;
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
