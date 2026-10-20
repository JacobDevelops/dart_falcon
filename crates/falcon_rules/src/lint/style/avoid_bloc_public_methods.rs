//! Enforce the package-confirmed Bloc convention.
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;
#[derive(Default)]
pub struct AvoidBlocPublicMethods;
impl Rule for AvoidBlocPublicMethods {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::style::avoid_bloc_public_fields::prepare(program, ctx, true)
    }
}
