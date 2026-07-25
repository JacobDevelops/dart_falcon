//! Use interpolation to compose strings and values.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct PreferInterpolationToComposeStrings;

impl Rule for PreferInterpolationToComposeStrings {
    fn name(&self) -> &'static str {
        "prefer-interpolation-to-compose-strings"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
