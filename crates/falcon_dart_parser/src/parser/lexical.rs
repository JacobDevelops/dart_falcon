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

/// Lex source into a lossless syntax tree without parsing declarations.
///
/// # Panics
///
/// Panics if `source` exceeds the compact token limit of `u32::MAX` bytes.
pub fn lex_tree(source: &str) -> falcon_syntax::cst::SyntaxTree {
    use falcon_syntax::cst::{Event, NodeKind, SyntaxTree};

    let syntax = lex_syntax(source);
    let mut regions = Vec::new();
    let tokens = cst_tokens_with_regions(&syntax, source, |span| regions.push(span));
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

pub(super) fn cst_tokens(syntax: &SyntaxStore, source: &str) -> Vec<Token> {
    cst_tokens_with_regions(syntax, source, |_| {})
}

fn cst_tokens_with_regions(
    syntax: &SyntaxStore,
    source: &str,
    mut visit: impl FnMut(Span),
) -> Vec<Token> {
    let mut output = Vec::with_capacity(syntax.tokens().len());
    let mut roots = syntax.tokens().iter().copied();
    let mut pending = Vec::new();
    let mut pieces = Vec::new();
    while let Some((token, store)) = pending
        .pop()
        .or_else(|| roots.next().map(|token| (token, syntax)))
    {
        if token.kind != TokenKind::StringLit {
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
                pieces.push((
                    Token::new(TokenKind::StringPart, cursor, start - cursor),
                    store,
                ));
            }
            pieces.push((Token::new(prefix, start, prefix_len), store));
            if prefix == TokenKind::InterpolationStart {
                if let Some(nested) = store.interpolation(span) {
                    pieces.extend(
                        nested
                            .tokens()
                            .iter()
                            .filter(|t| t.kind != TokenKind::Eof)
                            .map(|t| (*t, nested)),
                    );
                }
                pieces.push((Token::new(TokenKind::RBrace, span.end, 1), store));
                cursor = span.end + 1;
            } else {
                pieces.push((
                    Token::new(TokenKind::Ident, span.start, span.end - span.start),
                    store,
                ));
                cursor = span.end;
            }
        });
        if pieces.is_empty() {
            output.push(token);
        } else {
            let end = token.offset() + token.len();
            if cursor < end {
                pieces.push((
                    Token::new(TokenKind::StringPart, cursor, end - cursor),
                    store,
                ));
            }
            pending.extend(pieces.drain(..).rev());
        }
    }
    output
}

impl super::Parser<'_> {
    pub(super) fn scan_string_interpolations(
        &mut self,
        token: &Token,
    ) -> (
        Vec<falcon_syntax::ast::StringInterpolation>,
        Vec<super::ParseError>,
    ) {
        use falcon_syntax::ast::{Expr, Identifier, StringInterpolation};
        use falcon_syntax::cst::{Event, NodeKind};

        let mut regions = Vec::new();
        visit_interpolations(token.text(self.src), token.offset(), |region| {
            regions.push(region)
        });
        if regions.is_empty() {
            return (Vec::new(), Vec::new());
        }
        assert!(matches!(
            self.events.pop(),
            Some(Event::Token {
                kind: TokenKind::StringLit,
                ..
            })
        ));
        let mut cursor = token.offset();
        let mut interpolations = Vec::with_capacity(regions.len());
        let mut errors = Vec::new();
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
                let retained = self
                    .syntax
                    .interpolation(span)
                    .expect("retained interpolation stream");
                if span.start < span.end {
                    let mut sub = super::Parser::new(retained, self.src, self.version);
                    let expr = sub.parse_expr();
                    if sub.errors.is_empty()
                        && sub.at(TokenKind::Eof)
                        && !matches!(expr, Expr::Error { .. })
                    {
                        valid = true;
                        interpolations.push(StringInterpolation { expr, span });
                    }
                    self.events.extend(sub.events);
                    errors.extend(sub.errors);
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
                let expr = if name == "this" {
                    Expr::This { span }
                } else {
                    Expr::Ident(Identifier::new(name, span))
                };
                interpolations.push(StringInterpolation { expr, span });
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
        (interpolations, errors)
    }
}

#[cfg(test)]
#[path = "lexical_tests.rs"]
mod tests;
