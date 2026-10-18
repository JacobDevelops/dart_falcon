//! Avoid storing BuildContext in late fields or reading it in late initializers. Resolved Flutter context values are checked rather than identifier spelling.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidLateContext;

impl Rule for AvoidLateContext {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::flutter_state_rules::prepare(program, ctx, "avoid-late-context")
    }
}
