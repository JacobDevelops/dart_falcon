//! Don't rename parameters of overridden methods.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct AvoidRenamingMethodParameters;

impl Rule for AvoidRenamingMethodParameters {
    fn name(&self) -> &'static str {
        "avoid-renaming-method-parameters"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
