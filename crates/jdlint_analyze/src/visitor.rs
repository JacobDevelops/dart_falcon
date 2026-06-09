use jdlint_diagnostics::Diagnostic;
use jdlint_syntax::visitor::Visitor;

/// A visitor that accumulates diagnostics while traversing the AST.
///
/// Rules that need fine-grained node dispatch implement this trait instead of
/// (or in addition to) the simpler `Rule::analyze()` method. This trait composes
/// with the AST walker to intercept specific node kinds.
pub trait RuleVisitor: Visitor {
    /// Convert the accumulated visitor state into a flat list of diagnostics.
    fn into_diagnostics(self) -> Vec<Diagnostic>;
}
