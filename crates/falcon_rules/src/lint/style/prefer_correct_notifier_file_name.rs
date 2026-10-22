//! Requires snake_case file names ending in _notifier.dart for resolved Riverpod notifier classes.

use crate::lint::support::riverpod_naming::{Naming, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferCorrectNotifierFileName;

impl Rule for PreferCorrectNotifierFileName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Naming::NotifierFile)
    }
}
