//! Avoid `Future<Future<T>>`, `FutureOr<Future<T>>`, and other directly
//! nested future wrappers. Return `Future<T>` (or `FutureOr<T>`) instead.
//!
//! Checks explicit type occurrences, including expanded typedefs, using SDK
//! declaration identities and lexical type parameters. Collections of futures
//! are not nested wrappers; inferred-only types are deliberately not reported.
//!
//! Reference: https://dcm.dev/docs/rules/common/avoid-nested-futures/

use crate::lint::support::nested_async::{Nesting, prepare_nested};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidNestedFutures;

impl Rule for AvoidNestedFutures {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_nested(ctx, Nesting::Futures)
    }
}

