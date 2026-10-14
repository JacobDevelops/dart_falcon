//! Close instances of `dart:core` `Sink`.
//!
//! An unclosed sink can hold file handles, sockets, or buffered data and never
//! signal completion to its listeners:
//!
//! ```dart
//! void log(File file) {
//!   final sink = file.openWrite(); // BAD: never closed
//!   sink.write('entry');
//! }
//!
//! Future<void> log(File file) async {
//!   final sink = file.openWrite();
//!   sink.write('entry');
//!   await sink.close(); // GOOD
//! }
//! ```
//!
//! Fields and local variables whose type provably implements `Sink` (released
//! by `close`) or `dart:io` `Socket` (released by `close` or `destroy`) are
//! reported unless the file (fields) or enclosing function body (locals) calls
//! or tears off the release method on them, passes them to a call, returns
//! them, or hands them to another variable. Unresolved types stay silent.
//!
//! Adopted from the official `close_sinks` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

use crate::lint::resource_leaks::{LeakRule, Release};

#[derive(Default)]
pub struct CloseSinks;

const RULE: LeakRule = LeakRule {
    name: "close-sinks",
    message: "Unclosed instance of 'Sink'.",
    releases: &[
        Release {
            library: "dart:core",
            type_name: "Sink",
            method: "close",
        },
        Release {
            library: "dart:io",
            type_name: "Socket",
            method: "destroy",
        },
    ],
};

impl Rule for CloseSinks {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        RULE.analyze(program, ctx)
    }
}
