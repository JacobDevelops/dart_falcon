//! Explicitly tear-off `call` methods when using an object as a Function.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct ImplicitCallTearoffs;

impl Rule for ImplicitCallTearoffs {
    fn name(&self) -> &'static str {
        "implicit-call-tearoffs"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
