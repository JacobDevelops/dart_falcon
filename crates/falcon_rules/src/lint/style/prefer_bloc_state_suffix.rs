//! Enforce the package-confirmed Bloc convention.
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;
#[derive(Default)]
pub struct PreferBlocStateSuffix;
impl Rule for PreferBlocStateSuffix {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::bloc::prepare_models(
            program,
            ctx,
            crate::lint::bloc::ModelKind::State,
            crate::lint::bloc::ModelRequirement::Suffix,
            "prefer-bloc-state-suffix",
        )
    }
}
