//! Rejects direct Bloc and Cubit arguments to other blocs.

use super::bloc_passing::{Passing, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidPassingBlocToBloc;

impl Rule for AvoidPassingBlocToBloc {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Passing::BlocToBloc)
    }
}
