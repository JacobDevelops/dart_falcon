//! Requires owned `FocusNode` and Timer fields to be released in `State.dispose()`.
//!
//! Only fields constructed by a resolved Flutter `FocusNode` or dart:async Timer
//! constructor are checked, including assignments in lifecycle methods. A direct
//! `dispose()/cancel()` invocation in `dispose()` is required; tear-offs, callbacks,
//! and conditional-only invocations do not prove release. Borrowed and static
//! fields are ignored. Controllers belong to proper-controller-dispose; other
//! disposable fields belong to dispose-fields, and unretained temporary objects
//! belong to avoid-undisposed-instances.
//! Calls after a possible return/throw are not proof, including idempotence
//! guards whose cross-invocation state is outside this rule's analysis.

use crate::lint::support::controller_disposal::{DisposalKind, prepare_disposal};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;


#[derive(Default)]
pub struct DisposeFocusNodeAndTimer;

impl Rule for DisposeFocusNodeAndTimer {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_disposal(ctx, DisposalKind::FocusAndTimer)
    }
}
