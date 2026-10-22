//! Requires a Provider suffix on resolved Riverpod top-level providers and static fields.

use crate::lint::support::riverpod_naming::{Naming, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferRiverpodProviderSuffix;

impl Rule for PreferRiverpodProviderSuffix {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Naming::ProviderSuffix)
    }
}
