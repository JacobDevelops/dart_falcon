//! Avoid setState during build, initState, or didUpdateWidget. Synchronous local method calls are followed transitively; deferred closures and async methods are excluded.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidUnnecessarySetstate;

impl Rule for AvoidUnnecessarySetstate {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::flutter_state::prepare(program, ctx, "avoid-unnecessary-setstate")
    }
}
