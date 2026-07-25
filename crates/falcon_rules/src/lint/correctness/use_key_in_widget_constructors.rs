//! Use key in widget constructors.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct UseKeyInWidgetConstructors;

impl Rule for UseKeyInWidgetConstructors {
    fn name(&self) -> &'static str {
        "use-key-in-widget-constructors"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
