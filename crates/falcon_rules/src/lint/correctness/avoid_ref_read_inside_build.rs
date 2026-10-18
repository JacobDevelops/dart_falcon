//! Flags one-time Riverpod reads of provider state during widget build.
//!
//! Use watch to rebuild when state changes. Nested callbacks and notifier action reads are excluded.

use crate::lint::riverpod::{self, BuildRule};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidRefReadInsideBuild;

impl Rule for AvoidRefReadInsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        riverpod::prepare(program, ctx, BuildRule::ReadInside)
    }
}
