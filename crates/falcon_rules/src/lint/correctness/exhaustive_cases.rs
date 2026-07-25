//! Define case clauses for all constants in enum-like classes.
//!
//! Stub registration: the analysis is not implemented yet and emits no
//! diagnostics.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

pub struct ExhaustiveCases;

impl Rule for ExhaustiveCases {
    fn name(&self) -> &'static str {
        "exhaustive-cases"
    }

    fn analyze(&self, _program: &Program, _ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        Vec::new()
    }
}
