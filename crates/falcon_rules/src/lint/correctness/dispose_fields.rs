//! Requires other owned disposable State fields to be released in `dispose()`.
//!
//! Fields initialized or assigned a resolved constructor with an instance dispose
//! method must receive an eagerly evaluated `dispose()` call in the State's
//! `dispose()` method, including calls wrapped in arguments or operands.
//! Borrowed fields, static fields, unresolved types, and conditional-only releases
//! are not treated as proof of ownership or cleanup. `TextEditingController`,
//! `AnimationController`, `ScrollController`, `PageController`, and `TabController` belong
//! to proper-controller-dispose, FocusNode/Timer to dispose-focus-node-and-timer,
//! subscriptions to cancel-subscriptions, and sinks
//! to close-sinks. Unretained temporaries belong to avoid-undisposed-instances.
//! Calls after a possible return/throw are not proof, including idempotence
//! guards whose cross-invocation state is outside this rule's analysis.
//! Deferred closures and short-circuit right operands do not prove cleanup.

use crate::lint::support::controller_disposal::{DisposalKind, prepare_disposal};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct DisposeFields;

impl Rule for DisposeFields {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_disposal(ctx, DisposalKind::OtherFields)
    }
}
