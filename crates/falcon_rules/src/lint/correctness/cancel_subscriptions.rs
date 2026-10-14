//! Cancel instances of `dart:async` `StreamSubscription`.
//!
//! A subscription that is never cancelled keeps its stream (and everything the
//! listener captures) alive, leaking memory and running callbacks after their
//! owner is gone:
//!
//! ```dart
//! class Watcher {
//!   StreamSubscription<int>? _sub; // BAD: never cancelled
//! }
//!
//! class Watcher {
//!   StreamSubscription<int>? _sub;
//!   void dispose() => _sub?.cancel(); // GOOD
//! }
//! ```
//!
//! Fields and local variables whose type provably implements
//! `StreamSubscription` are reported unless the file (fields) or enclosing
//! function body (locals) calls or tears off `cancel` on them, passes them to a
//! call, returns them, or hands them to another variable. Unresolved types stay
//! silent.
//!
//! Adopted from the official `cancel_subscriptions` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

use crate::lint::resource_leaks::{LeakRule, Release};

#[derive(Default)]
pub struct CancelSubscriptions;

const RULE: LeakRule = LeakRule {
    name: "cancel-subscriptions",
    message: "Uncancelled instance of 'StreamSubscription'.",
    releases: &[Release {
        library: "dart:async",
        type_name: "StreamSubscription",
        method: "cancel",
    }],
};

impl Rule for CancelSubscriptions {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        RULE.analyze(program, ctx)
    }
}
