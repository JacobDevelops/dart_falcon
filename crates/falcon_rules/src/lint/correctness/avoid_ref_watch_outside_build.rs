//! Flags Riverpod WidgetRef.watch subscriptions outside synchronous widget build bodies.
//!
//! Subscriptions belong in build; event callbacks and other widget methods
//! should read once. Consumer builders and package-confirmed Flutter builder,
//! itemBuilder and separatorBuilder callbacks inside build are build contexts.
//! Provider Ref callbacks are left unchanged, including those inside widgets.

use crate::lint::riverpod::{self, BuildRule};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidRefWatchOutsideBuild;

impl Rule for AvoidRefWatchOutsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        riverpod::prepare(program, ctx, BuildRule::WatchOutside)
    }
}
