use falcon_analyze::AnalyzeContext;
use falcon_syntax::cst::*;

pub(crate) fn normalize<'a>(
    ctx: &'a AnalyzeContext<'_>,
    span: Span,
) -> impl Iterator<Item = u8> + 'a {
    use falcon_syntax::TokenKind;

    let mut line_comment = false;
    ctx.tree()
        .tokens_in(ctx.source(), span)
        .flat_map(move |token| {
            let mut text = token.text();
            let in_literal = token
                .parent()
                .ancestors()
                .any(|node| node.kind() == NodeKind::StringLitNode && span.contains(node.span()));
            let preserve_whitespace = if in_literal {
                true
            } else if line_comment {
                // The retained comment excludes its terminating newline; equality
                // includes that newline and any preceding CR.
                line_comment = token.kind() != TokenKind::Newline;
                true
            } else {
                if token.kind().is_comment() && text.starts_with("/* expect:") {
                    text = "";
                } else if matches!(token.kind(), TokenKind::LineComment | TokenKind::DocComment)
                    && text.starts_with("//")
                {
                    line_comment = true;
                }
                token.kind().is_comment()
                    || matches!(token.kind(), TokenKind::StringLit | TokenKind::StringPart)
            };
            text.bytes()
                .filter(move |byte| preserve_whitespace || !byte.is_ascii_whitespace())
        })
}
