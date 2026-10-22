//! `Future` results in `async` bodies must be awaited or marked `unawaited`.
//!
//! Forgetting `await` inside an `async` function lets the caller continue before
//! the work finishes and drops its errors:
//!
//! ```dart
//! Future<void> save() async {
//!   writeFile(); // BAD
//!   await writeFile(); // GOOD
//!   unawaited(writeFile()); // GOOD: explicitly fire-and-forget
//! }
//! ```
//!
//! Expression statements, cascade sections, and string interpolations whose
//! resolved type is `Future` (or a subtype) or `FutureOr` are reported inside
//! `async` and `async*` bodies. Assignments,
//! `Future.delayed(duration, computation)`, `Map.putIfAbsent`, and members
//! annotated `@awaitNotRequired` in this file are exempt; nullable or
//! unresolved types stay silent.
//!
//! Adopted from the official `unawaited_futures` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::support::unused_futures::{UnusedFuturesRule, unused_futures};

#[derive(Default)]
pub struct UnawaitedFutures;

impl Rule for UnawaitedFutures {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(unused_futures(
            program,
            ctx,
            &UnusedFuturesRule {
                in_async_bodies: true,
                future_message: "Add an `await` for the `Future` computed by this expression.",
                future_or_message: None,
            },
        ))
    }
}
