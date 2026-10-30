use falcon_syntax::cst::Span;
use falcon_syntax::token::{Token, TokenKind};

use crate::lexer::{Lexer, scan::interp_close};

/// Lex source into a lossless syntax tree without parsing declarations.
///
/// # Panics
///
/// Panics if `source` exceeds the compact token limit of `u32::MAX` bytes.
#[must_use]
pub fn lex_tree(source: &str) -> falcon_syntax::cst::SyntaxTree {
    use falcon_syntax::cst::{Event, NodeKind, SyntaxTree};

    let lexical = Lexer::new(source).tokenize();
    let mut regions = Vec::new();
    let tokens = cst_tokens_with_regions(&lexical, source, &[], None, |span| regions.push(span));
    if regions.is_empty() {
        return SyntaxTree::build(source, &tokens, Vec::new());
    }
    regions.sort_unstable_by_key(|span| span.start);
    let mut events = Vec::with_capacity(tokens.len() + 2 * (regions.len() + 1));
    events.push(Event::Start {
        kind: NodeKind::Program,
        forward_parent: None,
        offset: 0,
    });
    let mut interpolation_ends = Vec::with_capacity(regions.len());
    let mut regions = regions.into_iter();
    for token in &tokens {
        if token.kind == TokenKind::InterpolationStart {
            interpolation_ends.push(regions.next().expect("interpolation region").end);
            events.push(Event::Start {
                kind: NodeKind::StringInterpolation,
                forward_parent: None,
                offset: token.offset(),
            });
        }
        if !token.is_trivia() && token.kind != TokenKind::Eof {
            events.push(Event::Token {
                kind: token.kind,
                offset: token.offset(),
                len: token.len(),
            });
        }
        if token.kind == TokenKind::RBrace && interpolation_ends.last() == Some(&token.offset()) {
            interpolation_ends.pop();
            events.push(Event::Finish);
        }
    }
    events.push(Event::Finish);
    SyntaxTree::build(source, &tokens, events)
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

pub(super) fn cst_tokens(
    tokens: &[Token],
    source: &str,
    regions: &[(Span, Vec<Token>)],
) -> Vec<Token> {
    cst_tokens_with_regions(
        tokens,
        source,
        regions,
        Some(super::Parser::MAX_PARSE_DEPTH),
        |_| {},
    )
}

fn lex_region(source: &str, span: Span) -> Vec<Token> {
    Lexer::new(&source[span.start..span.end])
        .tokenize()
        .into_iter()
        .map(|token| Token::new(token.kind, token.offset() + span.start, token.len()))
        .collect()
}

fn cst_tokens_with_regions(
    tokens: &[Token],
    source: &str,
    regions: &[(Span, Vec<Token>)],
    depth_limit: Option<usize>,
    mut visit: impl FnMut(Span),
) -> Vec<Token> {
    let mut output = Vec::with_capacity(tokens.len());
    let mut roots = tokens.iter().copied().map(|token| (token, 0));
    let mut pending = Vec::new();
    let mut pieces = Vec::new();
    while let Some((token, depth)) = pending.pop().or_else(|| roots.next()) {
        if token.kind != TokenKind::StringLit || depth_limit.is_some_and(|limit| depth >= limit) {
            output.push(token);
            continue;
        }
        let mut cursor = token.offset();
        visit_interpolations(token.text(source), token.offset(), |region| {
            let (span, prefix, prefix_len) = match region {
                InterpolationRegion::Expression(span) => {
                    visit(span);
                    (span, TokenKind::InterpolationStart, 2)
                }
                InterpolationRegion::Identifier(span) => (span, TokenKind::InterpolationDollar, 1),
            };
            let start = span.start - prefix_len;
            if cursor < start {
                pieces.push(Token::new(TokenKind::StringPart, cursor, start - cursor));
            }
            pieces.push(Token::new(prefix, start, prefix_len));
            if prefix == TokenKind::InterpolationStart {
                let lexical;
                let cached = regions.binary_search_by_key(&span.start, |(span, _)| span.start);
                let tokens = if let Ok(index) = cached {
                    &regions[index].1
                } else {
                    lexical = lex_region(source, span);
                    &lexical
                };
                pieces.extend(tokens.iter().copied().filter(|t| t.kind != TokenKind::Eof));
                pieces.push(Token::new(TokenKind::RBrace, span.end, 1));
                cursor = span.end + 1;
            } else {
                pieces.push(Token::new(
                    TokenKind::Ident,
                    span.start,
                    span.end - span.start,
                ));
                cursor = span.end;
            }
        });
        if pieces.is_empty() {
            output.push(token);
        } else {
            let end = token.offset() + token.len();
            if cursor < end {
                pieces.push(Token::new(TokenKind::StringPart, cursor, end - cursor));
            }
            pending.extend(pieces.drain(..).rev().map(|token| (token, depth + 1)));
        }
    }
    output
}

impl super::Parser<'_> {
    pub(super) fn scan_string_interpolations(&mut self, token: &Token) {
        use falcon_syntax::cst::{Event, NodeKind};

        let mut regions = Vec::new();
        visit_interpolations(token.text(self.src), token.offset(), |region| {
            regions.push(region);
        });
        if regions.is_empty() {
            return;
        }
        assert!(matches!(
            self.events.pop(),
            Some(Event::Token {
                kind: TokenKind::StringLit,
                ..
            })
        ));
        let mut cursor = token.offset();
        for region in regions {
            let (span, prefix, prefix_len) = match region {
                InterpolationRegion::Expression(span) => (span, TokenKind::InterpolationStart, 2),
                InterpolationRegion::Identifier(span) => (span, TokenKind::InterpolationDollar, 1),
            };
            let start = span.start - prefix_len;
            if cursor < start {
                self.events.push(Event::Token {
                    kind: TokenKind::StringPart,
                    offset: cursor,
                    len: start - cursor,
                });
            }
            let marker = self.start_at(start);
            self.events.push(Event::Token {
                kind: prefix,
                offset: start,
                len: prefix_len,
            });
            let mut valid = prefix != TokenKind::InterpolationStart;
            if prefix == TokenKind::InterpolationStart {
                if span.start < span.end {
                    let tokens = lex_region(self.src, span);
                    let mut sub = super::Parser::new(&tokens, self.src, self.version);
                    sub.ctx.body = self.ctx.body;
                    sub.depth = self.depth;
                    sub.depth_limit_hit = self.depth_limit_hit;
                    let expr = sub.parse_expr();
                    if !sub.at(TokenKind::Eof) {
                        sub.error("expected '}' after interpolation expression");
                    }
                    if sub.errors.is_empty()
                        && sub.at(TokenKind::Eof)
                        && expr.kind != NodeKind::ErrorExpr
                    {
                        valid = true;
                    }
                    self.depth_limit_hit = sub.depth_limit_hit;
                    self.events.extend(sub.events);
                    self.errors.extend(sub.errors);
                    self.interpolation_tokens.extend(sub.interpolation_tokens);
                    self.interpolation_tokens.push((span, tokens));
                }
                self.events.push(Event::Token {
                    kind: TokenKind::RBrace,
                    offset: span.end,
                    len: 1,
                });
                cursor = span.end + 1;
            } else {
                let name = &self.src[span.start..span.end];
                let kind = if name == "this" {
                    NodeKind::ThisExpr
                } else {
                    NodeKind::Identifier
                };
                let ident = self.start_at(span.start);
                self.events.push(Event::Token {
                    kind: TokenKind::Ident,
                    offset: span.start,
                    len: span.end - span.start,
                });
                ident.complete(self, kind);
                cursor = span.end;
            }
            marker.complete(
                self,
                if valid {
                    NodeKind::StringInterpolation
                } else {
                    NodeKind::Error
                },
            );
        }
        let end = token.offset() + token.len();
        if cursor < end {
            self.events.push(Event::Token {
                kind: TokenKind::StringPart,
                offset: cursor,
                len: end - cursor,
            });
        }
    }
}

#[cfg(test)]
#[path = "lexical_tests.rs"]
mod tests;
