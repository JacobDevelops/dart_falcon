//! Don't use null check on a potentially nullable type parameter.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct NullCheckOnNullableTypeParameter;

impl Rule for NullCheckOnNullableTypeParameter {
    fn name(&self) -> &'static str {
        "null-check-on-nullable-type-parameter"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
