//! Flags a `Column`, `Row`, or `Flex` widget built with a single child.
//!
//! A multi-child layout widget that wraps exactly one child adds a layout node
//! and an allocation without changing what is rendered; the child can be used
//! directly, or swapped for `Align`, `Center`, or `Padding` if the axis
//! behavior actually mattered. The rule fires only when the `children` list
//! holds exactly one expression element, and it tolerates partially-parsed Dart
//! 3 collection elements by requiring a complete, `)`-terminated construction.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidSingleChildColumnOrRow;

impl Rule for AvoidSingleChildColumnOrRow {
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

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Some((name, args, span)) = crate::lint::flutter::widget_construction(node)
            && matches!(name, "Column" | "Row" | "Flex")
            // A recovery node can stop before the construction's closing token.
            && self.ctx.syntax.tokens_in(*span).last()
                .is_some_and(|token| token.kind == falcon_syntax::TokenKind::RParen)
        {
            for named in &args.named {
                if named.name.name == "children"
                    && let Expr::List { elements, .. } = named.value.unparenthesized()
                    && elements.len() == 1
                    && matches!(elements[0], CollectionElement::Expr(_))
                {
                    self.diags.push(self.ctx.diagnostic(
                        "avoid-single-child-column-or-row",
                        "Avoid a Column/Row/Flex with a single child; use the child directly.",
                        *span,
                    ));
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
