//! Flags `[0]` index access that should use the `.first` getter.
//!
//! Reading the leading element as `xs.first` states the intent directly and is
//! easier to read than the numeric `xs[0]`, which forces the reader to recognize
//! that zero means "first". The rule matches an index expression whose subscript
//! is the integer literal `0`; any other index, including a non-literal
//! expression that evaluates to zero, is left alone. Matching is syntactic on the
//! literal, so it does not confirm the receiver exposes a `first` getter.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferFirst;

impl Rule for PreferFirst {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Index { index, span, .. } = node
            && is_zero(index)
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-first",
                "Prefer .first over [0] to access the first element",
                *span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}

/// True for the integer literal `0`.
fn is_zero(expr: &Expr) -> bool {
    matches!(expr.unparenthesized(), Expr::IntLit { value, .. } if value == "0")
}
