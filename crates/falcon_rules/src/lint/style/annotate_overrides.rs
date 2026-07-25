//! Annotate overridden members.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct AnnotateOverrides;

impl Rule for AnnotateOverrides {
    fn name(&self) -> &'static str {
        "annotate-overrides"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
