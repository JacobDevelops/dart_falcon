use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;

use crate::AnalyzeContext;

/// Trait implemented by every lint rule.
///
/// Rule instances are immutable; diagnostics are local per file.
/// Thread safety: implementors MUST NOT use mutable self state.
pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>;
}
