//! Don't use constant patterns with type literals.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct TypeLiteralInConstantPattern;

impl Rule for TypeLiteralInConstantPattern {
    fn name(&self) -> &'static str {
        "type-literal-in-constant-pattern"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
