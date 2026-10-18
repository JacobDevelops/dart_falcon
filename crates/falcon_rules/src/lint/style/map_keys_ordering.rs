//! Flags literal string map keys that violate the configured order.
//!
//! Set `order` to `"ascending"` or `"descending"` for case-sensitive
//! lexicographic ordering. Without a valid explicit order, this rule is inactive.
//! Dynamic keys, spreads, collection control-flow, interpolations, adjacent
//! strings, and escaped non-raw strings break the comparison sequence.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::ordering::Order;

#[derive(Default)]
pub struct MapKeysOrdering;

const NAME: &str = "map-keys-ordering";

impl Rule for MapKeysOrdering {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(order) = Order::configured(ctx, NAME) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            order,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    order: Order,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, expr: Expr<'a>) -> StructuralTraversal {
        if let Expr::Map(map) = expr {
            let mut previous = None;
            for element in map.elements() {
                let key = match element {
                    MapElement::Entry(entry) => match entry.key() {
                        Expr::StringLit(string) if string.interpolations().next().is_none() => string
                            .delimiter()
                            .filter(|delimiter| delimiter.is_raw || !delimiter.content.contains('\\'))
                            .map(|delimiter| (delimiter.content, string.span())),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some((current, span)) = key
                    && previous.is_some_and(|previous| self.order.is_out_of_order(previous, current))
                {
                    self.diags.push(self.ctx.diagnostic(
                        NAME,
                        "Reorder literal string map keys to follow the configured order.",
                        span,
                    ));
                }
                previous = key.map(|(name, _)| name);
            }
        }
        StructuralTraversal::Continue
    }
}
}
