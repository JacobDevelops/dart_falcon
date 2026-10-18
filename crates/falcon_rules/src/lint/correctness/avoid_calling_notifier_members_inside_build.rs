//! Flags calls to Riverpod notifier methods during widget build.
//!
//! Build must not trigger notifier side effects. Method tear-offs and nested
//! callbacks are not calls during build; unrelated same-named types and Object
//! methods (`toString` and `noSuchMethod`) are ignored.

use crate::lint::riverpod::{self, BuildRule};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidCallingNotifierMembersInsideBuild;

impl Rule for AvoidCallingNotifierMembersInsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        riverpod::prepare(program, ctx, BuildRule::NotifierMembers)
    }
}
