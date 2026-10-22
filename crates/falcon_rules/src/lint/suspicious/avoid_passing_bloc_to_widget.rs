//! Rejects direct Bloc and Cubit arguments to Flutter widget constructors.

use crate::lint::support::bloc_passing::{Passing, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidPassingBlocToWidget;

impl Rule for AvoidPassingBlocToWidget {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Passing::BlocToWidget)
    }
}
