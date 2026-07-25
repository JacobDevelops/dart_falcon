//! Prefer declaring const constructors on `@immutable` classes.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct PreferConstConstructorsInImmutables;

impl Rule for PreferConstConstructorsInImmutables {
    fn name(&self) -> &'static str {
        "prefer-const-constructors-in-immutables"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
