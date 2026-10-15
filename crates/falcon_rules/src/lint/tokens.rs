//! Source-order significant tokens, and comment tokens, including those inside
//! string interpolations.
//!
//! The lexer yields a string literal as one token, so a rule that locates AST
//! nodes by token offset cannot see `(`, `[`, or `,` inside `'${f(a, b)}'`,
//! nor a comment inside `'${a /* c */}'`. [`significant_tokens`] and
//! [`comment_tokens`] re-lex every `${...}` region (recursively) and splice its
//! tokens right after the enclosing string token, keeping the list sorted by
//! offset.

use falcon_analyze::AnalyzeContext;
use falcon_dart_parser::lexer::Lexer;
use falcon_syntax::token::{Token, TokenKind};

/// Every non-trivia token of `source` in offset order, interpolations included.
pub(crate) fn significant_tokens(ctx: &AnalyzeContext) -> Vec<Token> {
    let mut out = Vec::new();
    push_tokens(ctx, Keep::Significant, &mut out);
    out
}

/// Every comment token of `source` in offset order, interpolations included:
/// the analyzer's token stream reaches comments inside `${...}` too.
pub(crate) fn comment_tokens(ctx: &AnalyzeContext) -> Vec<Token> {
    let mut out = Vec::new();
    push_tokens(ctx, Keep::Comments, &mut out);
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    Significant,
    Comments,
}

/// Pushes the kept source tokens, reusing the file's cached lexer output.
fn push_tokens(ctx: &AnalyzeContext, keep: Keep, out: &mut Vec<Token>) {
    for &token in ctx.tokens() {
        push(ctx.source, 0, token, keep, out);
    }
}

fn push(source: &str, base: usize, token: Token, keep: Keep, out: &mut Vec<Token>) {
    if token.kind == TokenKind::Eof {
        return;
    }
    let token = Token {
        offset: token.offset + base,
        ..token
    };
    let comment = matches!(
        token.kind,
        TokenKind::LineComment | TokenKind::BlockComment | TokenKind::DocComment
    );
    match keep {
        Keep::Significant if !token.is_trivia() => out.push(token),
        Keep::Comments if comment => out.push(token),
        _ => {}
    }
    if token.kind == TokenKind::StringLit {
        push_interpolations(source, token, keep, out);
    }
}

/// Pushes the tokens of each `${...}` region of the string literal `string`.
fn push_interpolations(source: &str, string: Token, keep: Keep, out: &mut Vec<Token>) {
    let text = string.text(source);
    if text.starts_with(['r', 'R']) {
        return;
    }
    let bytes = text.as_bytes();
    let end = string.offset + string.len;
    let mut index = 0;
    while index + 1 < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'$' if bytes[index + 1] == b'{' => {
                let inner = string.offset + index + 2;
                let close = visit_interpolation_tokens(&source[inner..end], |token| {
                    push(source, inner, token, keep, out);
                });
                let Some(close) = close.map(|offset| inner + offset) else {
                    return;
                };
                index = close + 1 - string.offset;
            }
            _ => index += 1,
        }
    }
}

/// Visit tokens after `${` up to its matching `}`, returning that brace's byte
/// offset. Braces in comments and nested strings do not close the interpolation.
pub(crate) fn visit_interpolation_tokens(
    source: &str,
    mut visit: impl FnMut(Token),
) -> Option<usize> {
    let mut lexer = Lexer::new(source);
    let mut depth = 0usize;
    loop {
        let token = lexer.next_token();
        match token.kind {
            TokenKind::Eof => return None,
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace if depth == 0 => return Some(token.offset),
            TokenKind::RBrace => depth -= 1,
            _ => {}
        }
        visit(token);
    }
}

#[cfg(test)]
mod tests {
    use super::{comment_tokens, significant_tokens};
    use falcon_analyze::AnalyzeContext;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn cached_source_tokens_preserve_interpolation_tokens_and_comments() {
        let source = "// outer\nfinal text = '${call(1 /* inner */)}';";
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config);
        let significant = significant_tokens(&ctx);
        assert!(significant.iter().any(|token| token.text(source) == "call"));
        assert!(significant.iter().any(|token| token.text(source) == "("));
        assert!(significant.iter().all(|token| !token.is_trivia()));
        let comments: Vec<_> = comment_tokens(&ctx)
            .into_iter()
            .map(|token| token.text(source))
            .collect();
        assert_eq!(comments, ["// outer", "/* inner */"]);
    }
}
