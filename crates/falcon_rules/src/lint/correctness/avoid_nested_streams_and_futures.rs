//! Avoid directly nesting SDK streams and futures: `Stream<Future<T>>` and
//! `Future<Stream<T>>` complicate consumption. Use `asyncMap`/`asyncExpand`, or
//! expose a single `Stream<T>` instead of a future that produces a stream.
//!
//! FutureOr is also future-like. Explicit type occurrences and typedef
//! expansions are checked with lexical type-parameter scopes; stream-of-stream
//! and future-of-future types are outside this rule. Inferred-only types are
//! not reported.
//!
//! Reference: https://dcm.dev/docs/rules/common/avoid-nested-streams-and-futures/

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::support::nested_async::{Nesting, prepare_nested};

#[derive(Default)]
pub struct AvoidNestedStreamsAndFutures;

impl Rule for AvoidNestedStreamsAndFutures {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_nested(ctx, Nesting::Mixed)
    }
}
