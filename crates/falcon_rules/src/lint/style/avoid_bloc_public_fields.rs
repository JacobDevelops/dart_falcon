//! Keep package-confirmed Bloc instance fields private; inherited API overrides are allowed.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation,

};

use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidBlocPublicFields;
impl Rule for AvoidBlocPublicFields {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, false)
    }
}

use crate::lint::support::bloc_public_members::prepare;
