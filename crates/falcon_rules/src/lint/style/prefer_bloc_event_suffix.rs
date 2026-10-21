//! Enforce the package-confirmed Bloc convention.
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;
#[derive(Default)]
pub struct PreferBlocEventSuffix;
impl Rule for PreferBlocEventSuffix {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::bloc::prepare_models(
            program,
            ctx,
            crate::lint::bloc::ModelKind::Event,
            crate::lint::bloc::ModelRequirement::Suffix,
        )
    }
}
