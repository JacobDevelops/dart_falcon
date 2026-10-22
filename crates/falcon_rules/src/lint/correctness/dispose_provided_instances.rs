//! Flags provider-created instances with a dispose method that lack ref.onDispose cleanup.
//!
//! Riverpod autoDispose destroys provider state, not arbitrary disposable objects.
//! Register each owned instance's dispose method with the provider's ref.onDispose.
//! Only package-confirmed Provider/FutureProvider/StreamProvider callbacks and
//! resolved constructor-created resources are checked. Nested deferred closures,
//! borrowed values, unknown types, and types without a recognized release method
//! are excluded. Direct cleanup and named local callbacks are recognized; opaque
//! cleanup calls mentioning the resource conservatively count as registration.
//! Borrowed factory returns are recognized only when their simple body is visible
//! in this file; constructor metadata does not expose bodies from other libraries.

use crate::lint::support::provider_resources::prepare_resources;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct DisposeProvidedInstances;

impl Rule for DisposeProvidedInstances {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_resources(program, ctx, true)
    }
}

