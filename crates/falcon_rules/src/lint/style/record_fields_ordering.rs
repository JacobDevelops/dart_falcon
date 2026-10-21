//! Flags named record type fields that violate the configured order.
//!
//! Set `order` to `"ascending"` or `"descending"` for case-sensitive
//! lexicographic ordering. Without a valid explicit order, this rule is inactive.
//! Named fields are checked independently for each record type. Positional fields
//! and record literals are ignored; reordering literal expressions can change effects.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::ordering::Order;

#[derive(Default)]
pub struct RecordFieldsOrdering;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Lexicographic direction in which named fields in each record type must be ordered.
    order: Option<Order>,
}

crate::lint::options::impl_options!(Options);


impl Rule for RecordFieldsOrdering {
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

    fn visit_dart_type(&mut self, ty: DartType<'a>) -> StructuralTraversal {
        if let DartType::Record(record) = ty {
            self.order.check_names(self.ctx, record.named().map(|field| Some(field.name())), &mut self.diags);
        }
        StructuralTraversal::Continue
    }
}
}
