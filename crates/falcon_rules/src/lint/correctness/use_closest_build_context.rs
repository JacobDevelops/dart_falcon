//! Use the nearest `BuildContext` parameter inside nested callbacks. Only resolved Flutter contexts are checked, avoiding unrelated variables named context.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct UseClosestBuildContext;

impl Rule for UseClosestBuildContext {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::flutter_state::prepare(program, ctx, "use-closest-build-context")
    }
}
