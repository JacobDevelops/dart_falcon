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

/// Compares token spelling, ignoring whitespace but preserving literal contents.
pub(crate) fn equivalent(left: &Expr, right: &Expr, ctx: &falcon_analyze::AnalyzeContext) -> bool {
    equivalent_spans(*left.span(), *right.span(), ctx)
}

fn equivalent_spans(left: Span, right: Span, ctx: &falcon_analyze::AnalyzeContext) -> bool {
    let tokens = |span: Span| {
        ctx.syntax
            .tokens_in(span)
            .filter(|token| !token.is_trivia() || token.kind.is_comment())
    };
    let mut left = tokens(left);
    let mut right = tokens(right);
    loop {
        let (left, right) = match (left.next(), right.next()) {
            (Some(left), Some(right)) => (left, right),
            (None, None) => return true,
            _ => return false,
        };
        if left.kind != right.kind {
            return false;
        }
        let same_spelling = if left.kind.is_comment() {
            left.text(ctx.source)
                .chars()
                .filter(|character| !character.is_whitespace())
                .eq(right
                    .text(ctx.source)
                    .chars()
                    .filter(|character| !character.is_whitespace()))
        } else if left.kind == falcon_syntax::TokenKind::StringLit {
            equivalent_strings(
                Span::new(left.offset(), left.offset() + left.len()),
                Span::new(right.offset(), right.offset() + right.len()),
                ctx,
            )
        } else {
            left.text(ctx.source) == right.text(ctx.source)
        };
        if !same_spelling {
            return false;
        }
    }
}

fn equivalent_strings(left: Span, right: Span, ctx: &falcon_analyze::AnalyzeContext) -> bool {
    let mut left_interpolations = ctx.syntax.interpolation_ranges(left);
    let mut right_interpolations = ctx.syntax.interpolation_ranges(right);
    let mut left_start = left.start;
    let mut right_start = right.start;
    loop {
        match (left_interpolations.next(), right_interpolations.next()) {
            (Some(left_inner), Some(right_inner)) => {
                if ctx.source[left_start..left_inner.start]
                    != ctx.source[right_start..right_inner.start]
                    || !equivalent_spans(left_inner, right_inner, ctx)
                {
                    return false;
                }
                left_start = left_inner.end;
                right_start = right_inner.end;
            }
            (None, None) => {
                return ctx.source[left_start..left.end] == ctx.source[right_start..right.end];
            }
            _ => return false,
        }
    }
}

/// Whether the parser retained an explicit grouping expression.
pub(crate) fn is_parenthesized(expr: &Expr) -> bool {
    matches!(expr, Expr::Paren { .. })
}

pub(crate) fn normalized(ctx: &falcon_analyze::AnalyzeContext, span: &Span) -> Option<String> {
    ctx.source.get(span.start..span.end)?;
    let mut text = String::new();
    append_normalized(ctx, *span, &mut text);
    Some(text)
}

fn append_normalized(ctx: &falcon_analyze::AnalyzeContext, span: Span, text: &mut String) {
    for token in ctx.syntax.tokens_in(span) {
        if token.kind == falcon_syntax::TokenKind::StringLit {
            let end = token.offset() + token.len();
            let mut start = token.offset();
            for inner in ctx.syntax.interpolation_ranges(Span::new(start, end)) {
                text.push_str(&ctx.source[start..inner.start]);
                append_normalized(ctx, inner, text);
                start = inner.end;
            }
            text.push_str(&ctx.source[start..end]);
        } else {
            text.extend(
                token
                    .text(ctx.source)
                    .chars()
                    .filter(|character| !character.is_whitespace()),
            );
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{equivalent_spans, normalized};
    use falcon_analyze::AnalyzeContext;
    use falcon_config::FalconConfig;
    use falcon_syntax::ast::Span;
    use std::path::Path;

    fn compare(left: &str, right: &str) -> bool {
        let source = format!("void f() {{ consume({left}, {right}); }}");
        let parsed = falcon_dart_parser::parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &source, &config, &parsed.syntax);
        let start = "void f() { consume(".len();
        let second = start + left.len() + 2;
        equivalent_spans(
            Span::new(start, start + left.len()),
            Span::new(second, second + right.len()),
            &ctx,
        )
    }

    #[test]
    fn token_equality_preserves_literal_spelling() {
        assert!(!compare("'a b'", "'ab'"));
        assert!(!compare("r'a b'", "r'ab'"));
        assert!(!compare("'''a\\nb'''", "'''ab'''"));
        assert!(compare("'a b'", "'a b'"));
    }

    #[test]
    fn token_equality_normalizes_only_interpolation_expression_whitespace() {
        assert!(compare("'${a}'", "'${ a }'"));
        assert!(compare("'${a + b}'", "'${ a+b }'"));
        assert!(compare(r#"'${"${a}"}'"#, r#"'${ "${ a }" }'"#));
        assert!(!compare("'a ${a}'", "'a${ a }'"));
        assert!(!compare(r#"'${"a b"}'"#, r#"'${"ab"}'"#));
    }

    #[test]
    fn normalization_preserves_literal_chunks_in_nested_interpolations() {
        let source = r#"final text = 'a ${ "b c ${ d }" } e';"#;
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        let span = Span::new("final text = ".len(), source.len() - 1);
        assert_eq!(
            normalized(&ctx, &span).as_deref(),
            Some(r#"'a ${"b c ${d}"} e'"#),
        );
    }

    #[test]
    fn token_equality_preserves_comment_policy() {
        assert!(compare("value + 1", "value+1"));
        assert!(!compare("value /* comment */ + 1", "value+1"));
        assert!(compare("value /* comment */ + 1", "value/*comment*/+1"));
    }
}
