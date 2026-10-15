//! Shared syntactic expression predicates and source views.

use falcon_syntax::ast::{BinaryOp, Expr, Span};

/// The analyzer `Identifier` name: `x` for a simple identifier and `a.b` for a
/// prefixed identifier whose prefix is itself a simple identifier.
pub(crate) fn identifier_name(expr: &Expr) -> Option<(&str, Option<&str>)> {
    match expr {
        Expr::Ident(identifier) => Some((identifier.name.as_str(), None)),
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } => match object.as_ref() {
            Expr::Ident(prefix) => Some((prefix.name.as_str(), Some(field.name.as_str()))),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn null_check(cond: &Expr) -> Option<(&Expr, bool)> {
    let Expr::Binary {
        op, left, right, ..
    } = cond.unparenthesized()
    else {
        return None;
    };
    let is_eq = match op {
        BinaryOp::EqEq => true,
        BinaryOp::NotEq => false,
        _ => return None,
    };
    match (left.unparenthesized(), right.unparenthesized()) {
        (Expr::NullLit { .. }, other) | (other, Expr::NullLit { .. }) => Some((other, is_eq)),
        _ => None,
    }
}

pub(crate) fn text(expr: &Expr, source: &str) -> String {
    normalized(source, expr.span()).expect("expression span must index source")
}

/// Whether the parser retained an explicit grouping expression.
pub(crate) fn is_parenthesized(expr: &Expr) -> bool {
    matches!(expr, Expr::Paren { .. })
}

/// Source text including wrapping parentheses, with whitespace removed.
pub(crate) fn paren_text(expr: &Expr, source: &str) -> String {
    normalized(source, expr.span()).expect("expression span must index source")
}

pub(crate) fn normalized(source: &str, span: &Span) -> Option<String> {
    Some(
        source
            .get(span.start..span.end)?
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect(),
    )
}

pub(crate) fn expr_src<'a>(expr: &Expr, source: &'a str) -> &'a str {
    let span = expr.span();
    &source[span.start..span.end.min(source.len())]
}

pub(crate) fn is_provably_non_null(expr: &Expr) -> bool {
    matches!(
        expr.unparenthesized(),
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::StringLit(_)
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. }
            | Expr::New { .. }
    )
}

pub(crate) fn line_of(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset.min(source.len())]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

pub(crate) fn scalar_text(expr: &Expr) -> Option<&str> {
    match expr.unparenthesized() {
        Expr::IntLit { value, .. } | Expr::DoubleLit { value, .. } => Some(value),
        Expr::Ident(id) => Some(&id.name),
        _ => None,
    }
}
