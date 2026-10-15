use falcon_syntax::ast::Span;
use falcon_syntax::token::{SyntaxStore, Token, TokenKind};

use crate::lexer::{Lexer, scan::interp_close};

/// Lex source without parsing, retaining trivia and identifiers inside string
/// interpolations. Each region gets one tokenization pass, using a heap-backed
/// work stack rather than recursion for nested interpolation expressions.
/// Scanning enclosing string literals still revisits nested literal contents.
///
/// # Panics
///
/// Panics if `source` exceeds the compact token limit of `u32::MAX` bytes.
pub fn lex_syntax(source: &str) -> SyntaxStore {
    let mut pending = vec![lex_region(source, Span::new(0, source.len()))];
    loop {
        if let Some(span) = pending.last_mut().unwrap().expressions.next() {
            pending.push(lex_region(source, span));
            continue;
        }
        let region = pending.pop().unwrap();
        let syntax = SyntaxStore::new(region.tokens)
            .with_interpolations(region.interpolations)
            .with_simple_interpolations(region.simple_interpolations);
        if let Some(parent) = pending.last_mut() {
            parent.interpolations.push((region.span, syntax));
        } else {
            return syntax;
        }
    }
}

struct LexedRegion {
    span: Span,
    tokens: Vec<Token>,
    expressions: std::vec::IntoIter<Span>,
    interpolations: Vec<(Span, SyntaxStore)>,
    simple_interpolations: Vec<Token>,
}

fn lex_region(source: &str, span: Span) -> LexedRegion {
    let mut tokens = Lexer::new(&source[span.start..span.end]).tokenize();
    let mut expressions = Vec::new();
    let mut simple_interpolations = Vec::new();
    for token in &mut tokens {
        *token = Token::new(
            token.kind,
            token
                .offset()
                .checked_add(span.start)
                .expect("token offset exceeds source capacity"),
            token.len(),
        );
        if token.kind == TokenKind::StringLit {
            visit_interpolations(token.text(source), token.offset(), |region| match region {
                InterpolationRegion::Expression(span) => {
                    expressions.push(span);
                }
                InterpolationRegion::Identifier(span) => {
                    simple_interpolations.push(Token::new(
                        TokenKind::Ident,
                        span.start,
                        span.end - span.start,
                    ));
                }
            });
        }
    }
    LexedRegion {
        span,
        tokens,
        expressions: expressions.into_iter(),
        interpolations: Vec::new(),
        simple_interpolations,
    }
}

pub(super) enum InterpolationRegion {
    Expression(Span),
    Identifier(Span),
}

/// Visit exactly the regions recognized by string parsing, excluding raw strings
/// and escaped dollars. Braced regions exclude their `${` and `}` delimiters.
pub(super) fn visit_interpolations(
    raw: &str,
    lit_start: usize,
    mut visit: impl FnMut(InterpolationRegion),
) {
    let bytes = raw.as_bytes();
    if bytes.first() == Some(&b'r') {
        return;
    }
    let dlen = if raw.starts_with("'''") || raw.starts_with("\"\"\"") {
        3
    } else if raw.starts_with('\'') || raw.starts_with('"') {
        1
    } else {
        return;
    };
    let content_end = raw.len().saturating_sub(dlen).max(dlen);
    let mut i = dlen;
    while i < content_end {
        match bytes[i] {
            b'\\' => i += 2,
            b'$' if i + 1 < content_end && bytes[i + 1] == b'{' => {
                if let Some(close) = interp_close(bytes, i + 1, content_end) {
                    visit(InterpolationRegion::Expression(Span::new(
                        lit_start + i + 2,
                        lit_start + close,
                    )));
                    i = close + 1;
                } else {
                    i += 1;
                }
            }
            b'$' if i + 1 < content_end && is_interp_ident_start(bytes[i + 1]) => {
                let id_start = i + 1;
                let mut end = id_start + 1;
                while end < content_end && is_interp_ident_continue(bytes[end]) {
                    end += 1;
                }
                visit(InterpolationRegion::Identifier(Span::new(
                    lit_start + id_start,
                    lit_start + end,
                )));
                i = end;
            }
            _ => i += 1,
        }
    }
}

// `$` separates adjacent interpolations rather than continuing an identifier.
fn is_interp_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_interp_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

#[cfg(test)]
#[path = "lexical_tests.rs"]
mod tests;
