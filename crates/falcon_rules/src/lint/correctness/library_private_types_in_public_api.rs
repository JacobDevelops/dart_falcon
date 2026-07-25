//! Avoid using private types in public APIs.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct LibraryPrivateTypesInPublicApi;

impl Rule for LibraryPrivateTypesInPublicApi {
    fn name(&self) -> &'static str {
        "library-private-types-in-public-api"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
