//! Require super.dispose() in Flutter State dispose overrides. Presence is checked separately from the ordering rule; nested callbacks do not count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct RequireSuperDispose;

impl Rule for RequireSuperDispose {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::flutter_state_rules::prepare(program, ctx, "require-super-dispose")
    }
}
