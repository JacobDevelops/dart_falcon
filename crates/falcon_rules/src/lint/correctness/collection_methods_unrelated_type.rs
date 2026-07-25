//! Invocation of collection methods with arguments of unrelated types.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct CollectionMethodsUnrelatedType;

impl Rule for CollectionMethodsUnrelatedType {
    fn name(&self) -> &'static str {
        "collection-methods-unrelated-type"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
