//! Requires snake_case file names ending in _provider.dart for resolved Riverpod provider declarations.

use super::prefer_riverpod_notifier_suffix::{Naming, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferCorrectProviderFileName;

impl Rule for PreferCorrectProviderFileName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Naming::ProviderFile)
    }
}
