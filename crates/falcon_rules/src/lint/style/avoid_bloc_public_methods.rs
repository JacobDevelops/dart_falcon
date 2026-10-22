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
        crate::lint::support::bloc_public_members::prepare(program, ctx, true)
    }
}
