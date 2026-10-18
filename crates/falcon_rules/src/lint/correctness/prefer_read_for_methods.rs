//! Flags void method actions on a listening Provider lookup during build.
//!
//! Use context.read or Provider.of(listen: false) for actions instead. Only
//! resolved void methods in confirmed widget build/buildWithChild methods are
//! checked; queries, property reads and unresolved values keep subscriptions.
//! Widget helpers and didChangeDependencies are not assumed to run in build.
//! Outside build, avoid-watch-outside-build owns the diagnostic.

use super::avoid_read_inside_build::{Check, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct PreferReadForMethods;

impl Rule for PreferReadForMethods {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Check::ReadForMethods)
    }
}
