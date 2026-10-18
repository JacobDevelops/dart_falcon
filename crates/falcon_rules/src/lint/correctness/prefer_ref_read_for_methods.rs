//! Flags watching a Riverpod notifier solely to invoke one of its methods.
//!
//! Actions should read the notifier rather than establish a rebuild subscription. Only package-confirmed Ref receivers and notifier types are matched.

use crate::lint::riverpod::{self, BuildRule};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferRefReadForMethods;

impl Rule for PreferRefReadForMethods {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        riverpod::prepare(program, ctx, BuildRule::ReadForMethods)
    }
}
