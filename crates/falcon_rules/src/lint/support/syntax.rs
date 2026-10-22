//! Syntax identity and literal classification.

use falcon_syntax::cst::{self, Expr};
/// Token comparison preserves boundaries and literal contents without allocating.
pub(crate) fn same_syntax(left: cst::SyntaxNode<'_>, right: cst::SyntaxNode<'_>) -> bool {
    left.significant_tokens()
        .map(|token| (token.kind(), token.text()))
        .eq(right
            .significant_tokens()
            .map(|token| (token.kind(), token.text())))
}
pub(crate) fn scalar_literal(expression: Expr<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) => true,
        Expr::StringLit(string) => string.interpolations().next().is_none(),
        _ => false,
    }
}
