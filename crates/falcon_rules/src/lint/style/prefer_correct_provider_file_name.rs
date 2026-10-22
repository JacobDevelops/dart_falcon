//! Requires snake_case file names ending in _provider.dart for resolved Riverpod provider declarations, including annotated generator functions.

use crate::lint::support::riverpod_naming::{Naming, prepare};
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
