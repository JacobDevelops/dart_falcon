//! Flags listening Provider accesses proven to run outside Flutter builds.
//!
//! Provider watch/select and listening Provider.of calls subscribe to widget
//! updates. Use read in initState, dispose, non-widget classes and deferred
//! callbacks. didChangeDependencies may establish dependencies; buildWithChild
//! on nested's SingleChildStatelessWidget/SingleChildState is a build entry.
//! Only confirmed Flutter `BuildContext` receivers with provider imports match.
//! Widget helpers, unresolved bases and callback arguments with an unknown
//! execution phase are skipped. Named on… callbacks are deferred events.

use crate::lint::support::provider_access::{Check, prepare};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidWatchOutsideBuild;

impl Rule for AvoidWatchOutsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Check::WatchOutsideBuild)
    }
}
