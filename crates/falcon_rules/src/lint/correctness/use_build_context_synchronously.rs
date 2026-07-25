//! Do not use BuildContexts across async gaps.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct UseBuildContextSynchronously;

impl Rule for UseBuildContextSynchronously {
    fn name(&self) -> &'static str {
        "use-build-context-synchronously"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
