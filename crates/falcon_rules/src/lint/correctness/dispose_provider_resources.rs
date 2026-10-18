//! Flags provider-created resources without autoDispose or ref.onDispose cleanup.
//!
//! Resources created in a long-lived Riverpod provider need an explicit lifetime.
//! Use an autoDispose provider or register the resource's cleanup with ref.onDispose.
//! Checks package-confirmed Provider, FutureProvider and StreamProvider callbacks
//! for resolved constructor-created instances with a zero-required-argument
//! close or cancel method. Dispose types are left to dispose-provided-instances.
//! Borrowed values, deferred closures and unknown types are excluded. This
//! lifetime policy is separate from disposing arbitrary provided instances:
//! autoDispose alone does not invoke an object's dispose.
//! SDK sinks, sockets and subscriptions are left to the shipped leak rules unless
//! returned as provider state. Simple source-visible factories returning borrowed
//! values are excluded, while allocating factories remain eligible.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct DisposeProviderResources;

impl Rule for DisposeProviderResources {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        super::dispose_provided_instances::prepare_resources(program, ctx, false)
    }
}
