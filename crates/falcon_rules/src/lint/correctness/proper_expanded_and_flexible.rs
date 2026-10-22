//! Require `Expanded` and `Flexible` to sit directly inside a flex widget.
//!
//! Flags an `Expanded` or `Flexible` passed as the `child:` of a widget that is
//! not a `Row`, `Column`, or `Flex`. These widgets report their flex factor to a
//! surrounding flex container through the render tree; under any other parent
//! they have no flex parent to negotiate with and throw a layout error at
//! runtime. Put them in the `children:` of a `Row`, `Column`, or `Flex`, or
//! remove the wrapper. The rule reports only when the illegal parent is directly
//! visible at the construction site.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ProperExpandedAndFlexible;

impl Rule for ProperExpandedAndFlexible {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
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
        // Only flag when we can positively see an illegal parent context: an
        // Expanded/Flexible passed as the `child:` of a non-Flex widget. A Flex
        // widget (Row/Column/Flex) receives its children via `children:`, so an
        // Expanded there is legal and not considered here.
        if let Some((parent, args, _)) = crate::lint::flutter::widget_construction(node)
            && !matches!(parent, "Row" | "Column" | "Flex")
        {
            for named in args.named() {
                if named.name().name() == "child"
                    && let Some((child, _, child_span)) =
                        crate::lint::flutter::widget_construction(named.value())
                    && matches!(child, "Expanded" | "Flexible")
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Expanded and Flexible must be direct children of a Row, Column, or Flex.",
                        child_span,
                    ));
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
