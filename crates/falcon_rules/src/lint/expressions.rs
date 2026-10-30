//! Shared syntactic expression predicates and source views.

use falcon_syntax::cst::{BinaryOp, Expr, Span};

pub(crate) fn stable_local(name: &str, state: &falcon_analyze::SemanticState<'_>) -> bool {
    state.environment.binding_declaration(name).is_some()
        && !state.is_own_field(name)
        && state.is_variable_read(name)
}

pub(crate) fn local_name<'a>(
    expression: Expr<'a>,
    state: &falcon_analyze::SemanticState<'a>,
) -> Option<&'a str> {
    let Expr::Ident(identifier) = expression.unparenthesized() else {
        return None;
    };
    stable_local(identifier.name(), state).then_some(identifier.name())
}

pub(crate) fn integer_literal(expression: Expr<'_>) -> Option<i128> {
    let (literal, negative) = match expression.unparenthesized() {
        Expr::IntLit(literal) => (literal, false),
        Expr::Unary(unary) if unary.op() == falcon_syntax::cst::UnaryOp::Minus => {
            let Expr::IntLit(literal) = unary.operand().unparenthesized() else {
                return None;
            };
            (literal, true)
        }
        _ => return None,
    };
    let value = i128::from(literal.value().parse::<i64>().ok()?);
    Some(if negative { -value } else { value })
}

pub(crate) fn nested_integer_literal(mut expression: Expr<'_>) -> Option<i128> {
    let mut negative = false;
    while let Expr::Unary(unary) = expression.unparenthesized() {
        if unary.op() != falcon_syntax::cst::UnaryOp::Minus {
            return None;
        }
        negative = !negative;
        expression = unary.operand();
    }
    let value = integer_literal(expression)?;
    Some(if negative { -value } else { value })
}

pub(crate) fn reverse(op: BinaryOp) -> Option<BinaryOp> {
    Some(match op {
        BinaryOp::EqEq | BinaryOp::NotEq => op,
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::LtEq => BinaryOp::GtEq,
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::GtEq => BinaryOp::LtEq,
        _ => return None,
    })
}

pub(crate) fn interval(op: BinaryOp, bound: i128) -> Option<(i128, i128, Option<i128>)> {
    Some(match op {
        BinaryOp::EqEq => (bound, bound, None),
        BinaryOp::NotEq => (i128::MIN, i128::MAX, Some(bound)),
        BinaryOp::Lt => (i128::MIN, bound - 1, None),
        BinaryOp::LtEq => (i128::MIN, bound, None),
        BinaryOp::Gt => (bound + 1, i128::MAX, None),
        BinaryOp::GtEq => (bound, i128::MAX, None),
        _ => return None,
    })
}

/// The analyzer `Identifier` name: `x` for a simple identifier and `a.b` for a
/// prefixed identifier whose prefix is itself a simple identifier.
pub(crate) fn identifier_name(expr: Expr<'_>) -> Option<(&str, Option<&str>)> {
    match expr {
        Expr::Ident(identifier) => Some((identifier.name(), None)),
        Expr::Field(field) if !field.is_null_safe() => match field.object() {
            Expr::Ident(prefix) => Some((prefix.name(), Some(field.field().name()))),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn null_check(cond: Expr<'_>) -> Option<(Expr<'_>, bool)> {
    let Expr::Binary(binary) = cond.unparenthesized() else {
        return None;
    };
    let is_eq = match binary.op() {
        BinaryOp::EqEq => true,
        BinaryOp::NotEq => false,
        _ => return None,
    };
    match (
        binary.left().unparenthesized(),
        binary.right().unparenthesized(),
    ) {
        (Expr::NullLit(_), other) | (other, Expr::NullLit(_)) => Some((other, is_eq)),
        _ => None,
    }
}

/// Compares token spelling, ignoring whitespace but preserving literal contents.
pub(crate) fn equivalent(
    left: Expr<'_>,
    right: Expr<'_>,
    ctx: &falcon_analyze::AnalyzeContext,
) -> bool {
    equivalent_spans(left.span(), right.span(), ctx)
}

fn equivalent_spans(left: Span, right: Span, ctx: &falcon_analyze::AnalyzeContext) -> bool {
    let tokens = |span: Span| {
        ctx.tree()
            .tokens_in(ctx.source(), span)
            .filter(|token| !token.is_trivia() || token.kind().is_comment())
    };
    let mut left = tokens(left);
    let mut right = tokens(right);
    loop {
        let (left, right) = match (left.next(), right.next()) {
            (Some(left), Some(right)) => (left, right),
            (None, None) => return true,
            _ => return false,
        };
        if left.kind() != right.kind() {
            return false;
        }
        let same_spelling = if left.kind().is_comment() {
            left.text()
                .chars()
                .filter(|character| !character.is_whitespace())
                .eq(right
                    .text()
                    .chars()
                    .filter(|character| !character.is_whitespace()))
        } else {
            left.text() == right.text()
        };
        if !same_spelling {
            return false;
        }
    }
}

/// Whether the parser retained an explicit grouping expression.
pub(crate) fn is_parenthesized(expr: Expr<'_>) -> bool {
    matches!(expr, Expr::Paren(_))
}

pub(crate) fn normalized(ctx: &falcon_analyze::AnalyzeContext, span: &Span) -> Option<String> {
    ctx.source().get(span.start..span.end)?;
    let mut text = String::new();
    append_normalized(ctx, *span, &mut text);
    Some(text)
}

fn append_normalized(ctx: &falcon_analyze::AnalyzeContext, span: Span, text: &mut String) {
    for token in ctx.tree().tokens_in(ctx.source(), span) {
        if matches!(
            token.kind(),
            falcon_syntax::TokenKind::StringLit | falcon_syntax::TokenKind::StringPart
        ) {
            text.push_str(token.text());
        } else {
            text.extend(
                token
                    .text()
                    .chars()
                    .filter(|character| !character.is_whitespace()),
            );
        }
    }
}

pub(crate) fn expr_src<'a>(expr: Expr<'_>, source: &'a str) -> &'a str {
    let span = expr.span();
    &source[span.start..span.end.min(source.len())]
}

pub(crate) fn is_provably_non_null(expr: Expr<'_>) -> bool {
    matches!(
        expr.unparenthesized(),
        Expr::IntLit(_)
            | Expr::DoubleLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::List(_)
            | Expr::Map(_)
            | Expr::Set(_)
            | Expr::Record(_)
            | Expr::New(_)
    )
}

pub(crate) fn line_of(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset.min(source.len())]
        .split(|&byte| byte == b'\n')
        .count()
}

pub(crate) fn scalar_text(expr: Expr<'_>) -> Option<&str> {
    match expr.unparenthesized() {
        Expr::IntLit(lit) => Some(lit.value()),
        Expr::DoubleLit(lit) => Some(lit.value()),
        Expr::Ident(id) => Some(id.name()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{equivalent_spans, normalized};
    use falcon_analyze::AnalyzeContext;
    use falcon_config::FalconConfig;
    use falcon_syntax::cst::Span;
    use std::path::Path;

    fn compare(left: &str, right: &str) -> bool {
        let source = format!("void f() {{ consume({left}, {right}); }}");
        let parsed = falcon_dart_parser::parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into_boxed_str(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
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
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
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
