//! Shared syntactic expression predicates and source views.

use falcon_syntax::ast::{BinaryOp, Expr, Span};

pub(crate) fn null_check(cond: &Expr) -> Option<(&Expr, bool)> {
    let Expr::Binary {
        op, left, right, ..
    } = cond
    else {
        return None;
    };
    let is_eq = match op {
        BinaryOp::EqEq => true,
        BinaryOp::NotEq => false,
        _ => return None,
    };
    match (left.as_ref(), right.as_ref()) {
        (Expr::NullLit { .. }, other) | (other, Expr::NullLit { .. }) => Some((other, is_eq)),
        _ => None,
    }
}

pub(crate) fn text(expr: &Expr, source: &str) -> String {
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
        expr,
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
    match expr {
        Expr::IntLit { value, .. } | Expr::DoubleLit { value, .. } => Some(value),
        Expr::Ident(id) => Some(&id.name),
        _ => None,
    }
}
