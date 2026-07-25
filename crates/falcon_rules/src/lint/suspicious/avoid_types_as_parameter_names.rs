//! Avoid types as parameter names.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct AvoidTypesAsParameterNames;

impl Rule for AvoidTypesAsParameterNames {
    fn name(&self) -> &'static str {
        "avoid-types-as-parameter-names"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
