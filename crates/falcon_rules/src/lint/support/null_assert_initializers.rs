use crate::lint::declarations::declaration_initializers as all_declaration_initializers;
use falcon_syntax::cst::*;

/// Null assertions directly initializing top-level variables and fields; the
/// walker supplies their declared type as the expected type. Return the inner
/// assertion span when the initializer is parenthesized.
pub(crate) fn declaration_initializers(program: Program<'_>) -> Vec<Span> {
    all_declaration_initializers(program)
        .filter(|initializer| matches!(initializer.unparenthesized(), Expr::NullAssert(_)))
        .map(|initializer| initializer.unparenthesized().span())
        .collect()
}
