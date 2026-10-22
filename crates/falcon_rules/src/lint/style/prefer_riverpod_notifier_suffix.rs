//! Requires a `Notifier` suffix on classes extending resolved Riverpod notifiers.
//! Package identities, including inherited identities and re-exports, distinguish
//! Riverpod notifiers from lookalikes. Generated Dart files are ignored.



use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};

use falcon_syntax::cst::Program;


#[derive(Default)]
pub struct PreferRiverpodNotifierSuffix;

impl Rule for PreferRiverpodNotifierSuffix {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Naming::NotifierSuffix)
    }
}

use crate::lint::support::riverpod_naming::{Naming, prepare};
