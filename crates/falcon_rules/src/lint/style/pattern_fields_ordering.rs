//! Flags named object and record pattern fields that violate the configured order.
//!
//! Set `order` to `"ascending"` or `"descending"` for case-sensitive
//! lexicographic ordering. Without a valid explicit order, this rule is inactive.
//! Explicit labels and inferred shorthand names are compared; positional record
//! fields and unresolved shorthand names break the comparison sequence.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::ordering::Order;

#[derive(Default)]
pub struct PatternFieldsOrdering;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Alphabetical order for named pattern fields; unset leaves this rule inactive.
    order: Option<Order>,
}

crate::lint::options::impl_options!(Options);


impl Rule for PatternFieldsOrdering {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(order) = crate::lint::options::get::<Options>(ctx, Self::NAME).order else {
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

    fn visit_pattern(&mut self, pattern: Pattern<'a>) -> StructuralTraversal {
        match pattern {
            Pattern::Object(object) => self.order.check_names(
                self.ctx,
                object.fields().map(|field| {
                    let name = field.name();
                    (!name.name().starts_with('<')).then_some(name)
                }),
                &mut self.diags,
            ),
            Pattern::Record(record) => self.order.check_names(
                self.ctx, record.fields().map(|field| field.name()), &mut self.diags,
            ),
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}
