//! Avoid inherited-widget subscriptions during initState. Checks direct BuildContext dependency calls and known subscribing Flutter of methods, including synchronous local helper calls.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidInheritedWidgetInInitstate;

impl Rule for AvoidInheritedWidgetInInitstate {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::flutter_state_rules::prepare(program, ctx, "avoid-inherited-widget-in-initstate")
    }
}
