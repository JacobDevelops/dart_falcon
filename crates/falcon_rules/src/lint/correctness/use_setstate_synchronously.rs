//! Require a mounted check before calling setState after an async gap. Reuses the BuildContext async-state analysis and its control-flow guards.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct UseSetstateSynchronously;

impl Rule for UseSetstateSynchronously {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::use_build_context_synchronously::prepare_setstate(program, ctx)
    }
}
