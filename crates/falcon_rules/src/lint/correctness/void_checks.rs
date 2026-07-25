//! Don't assign to void.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct VoidChecks;

impl Rule for VoidChecks {
    fn name(&self) -> &'static str {
        "void-checks"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
