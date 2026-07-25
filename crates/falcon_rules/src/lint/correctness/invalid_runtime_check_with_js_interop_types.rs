//! Avoid is/as runtime checks on JS interop types (unsound).
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct InvalidRuntimeCheckWithJsInteropTypes;

impl Rule for InvalidRuntimeCheckWithJsInteropTypes {
    fn name(&self) -> &'static str {
        "invalid-runtime-check-with-js-interop-types"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
