//! `==` invocation with references of unrelated types.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct UnrelatedTypeEqualityChecks;

impl Rule for UnrelatedTypeEqualityChecks {
    fn name(&self) -> &'static str {
        "unrelated-type-equality-checks"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
