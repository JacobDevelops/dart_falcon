//! Flags enum constants whose declared names violate the configured order.
//!
//! Set `order` to `"ascending"` or `"descending"` to compare names
//! lexicographically and case-sensitively. Without a valid explicit order,
//! this rule is inactive. Constructor arguments and enum members do not affect
//! the comparison.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::ordering::Order;

#[derive(Default)]
pub struct EnumConstantsOrdering;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Alphabetical order for enum constants; unset leaves this rule inactive.
    order: Option<Order>,
}

crate::lint::options::impl_options!(Options);


impl Rule for EnumConstantsOrdering {
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

    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Enum(enumeration) = decl {
            self.order.check_names(self.ctx, enumeration.variants().map(|variant| Some(variant.name())), &mut self.diags);
        }
        StructuralTraversal::Continue
    }
}
}
