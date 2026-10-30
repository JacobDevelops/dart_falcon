//! Flags Riverpod WidgetRef.read calls after an async gap without a mounted guard.
//!
//! A widget may be disposed while awaiting a future. Check Flutter mounted before
//! reading a provider. Only resolved Riverpod `WidgetRef` receivers are checked;
//! notifier references are handled by use-ref-and-state-synchronously. Nested
//! function bodies form independent async boundaries.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::riverpod_async::{self, Target};

#[derive(Default)]
pub struct UseRefReadSynchronously;

impl Rule for UseRefReadSynchronously {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        riverpod_async::prepare(program, ctx, Target::WidgetRead)
    }
}
