//! Flags notifier ref and state accesses after an async gap without ref.mounted.
//!
//! A notifier may be disposed while its asynchronous method is suspended. Guard
//! subsequent accesses with ref.mounted. Only Riverpod 3 AnyNotifier-declared
//! ref and state members are checked; older APIs without ref.mounted, shadowed
//! locals, unrelated classes, and mounted checks themselves are excluded.
//! Nested function bodies form independent async boundaries.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::riverpod_async::{self, Target};

#[derive(Default)]
pub struct UseRefAndStateSynchronously;

impl Rule for UseRefAndStateSynchronously {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        riverpod_async::prepare(program, ctx, Target::Notifier)
    }
}
