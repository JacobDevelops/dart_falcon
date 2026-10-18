//! Avoid setState callbacks with empty block bodies. Comments and empty statements do not update state; unrelated methods are ignored.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidEmptySetstate;

impl Rule for AvoidEmptySetstate {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::flutter_state_rules::prepare(program, ctx, "avoid-empty-setstate")
    }
}
