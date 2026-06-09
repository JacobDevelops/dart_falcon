//! Dart 3.x recursive-descent parser.

mod decl;
mod expr;
mod pattern;
mod stmt;
mod types;

use jdlint_syntax::ast::*;
use jdlint_syntax::token::{Token, TokenKind};

use crate::lexer::{filter_trivia, Lexer};

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse Dart 3.x source into a [`Program`] AST.
///
/// Returns the AST and any non-fatal parse errors encountered. The parser
/// always produces an AST (possibly with [`TopLevelDecl::Error`] nodes) and
/// never panics.
pub fn parse(source: &str) -> (Program, Vec<ParseError>) {
    let raw_tokens = Lexer::new(source).tokenize();
    let tokens = filter_trivia(raw_tokens);
    let mut p = Parser::new(tokens, source);
    let program = p.parse_program();
    (program, p.errors)
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

// ── Parser struct ─────────────────────────────────────────────────────────────

pub(super) struct Parser<'src> {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) src: &'src str,
    pub(super) errors: Vec<ParseError>,
}

impl<'src> Parser<'src> {
    pub(super) fn new(tokens: Vec<Token>, src: &'src str) -> Self {
        Self { tokens, pos: 0, src, errors: Vec::new() }
    }

    // ── Cursor ────────────────────────────────────────────────────────────────

    pub(super) fn cur(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| self.tokens.last().unwrap())
    }

    pub(super) fn peek(&self, offset: usize) -> &Token {
        let idx = (self.pos + offset).min(self.tokens.len().saturating_sub(1));
        &self.tokens[idx]
    }

    pub(super) fn at(&self, kind: TokenKind) -> bool {
        self.cur().kind == kind
    }

    pub(super) fn at_any(&self, kinds: &[TokenKind]) -> bool {
        kinds.contains(&self.cur().kind)
    }

    pub(super) fn advance(&mut self) -> Token {
        let tok = self.cur().clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    pub(super) fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) { Some(self.advance()) } else { None }
    }

    pub(super) fn expect(&mut self, kind: TokenKind) -> Token {
        if self.at(kind.clone()) {
            self.advance()
        } else {
            self.error(format!(
                "expected {:?}, got {:?}",
                kind,
                self.cur().kind
            ));
            self.cur().clone()
        }
    }

    pub(super) fn error(&mut self, msg: impl Into<String>) {
        let offset = self.cur().offset;
        self.errors.push(ParseError { message: msg.into(), offset });
    }

    /// Advance past tokens until we reach one of the synchronisation tokens.
    pub(super) fn synchronize(&mut self, stops: &[TokenKind]) {
        while !self.at(TokenKind::Eof) && !self.at_any(stops) {
            self.advance();
        }
    }

    pub(super) fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.cur().offset)
    }

    pub(super) fn cur_span(&self) -> Span {
        let t = self.cur();
        Span::new(t.offset, t.offset + t.len)
    }

    pub(super) fn tok_span(tok: &Token) -> Span {
        Span::new(tok.offset, tok.offset + tok.len)
    }

    pub(super) fn tok_text<'a>(&'a self, tok: &Token) -> &'a str {
        &self.src[tok.offset..tok.offset + tok.len]
    }

    pub(super) fn cur_text(&self) -> &str {
        self.tok_text(self.cur())
    }

    // ── Identifier helpers ────────────────────────────────────────────────────

    /// Accept a keyword token as an identifier (built-in identifiers, contextual).
    pub(super) fn is_ident_like(&self) -> bool {
        use TokenKind::*;
        matches!(
            self.cur().kind,
            Ident | Abstract | As | Base | Covariant | Deferred | Dynamic | Export
                | Extension | External | Factory | Function | Get | Hide | Implements
                | Import | Interface | Late | Library | Mixin | Operator | Part
                | Required | Sealed | Set | Show | Static | Type | Typedef
                | Async | Await | Sync | Yield | When | On | Override
        )
    }

    pub(super) fn parse_ident(&mut self) -> Option<Identifier> {
        if self.is_ident_like() {
            let tok = self.advance();
            let name = self.tok_text(&tok).to_string();
            Some(Identifier::new(name, Self::tok_span(&tok)))
        } else {
            None
        }
    }

    pub(super) fn expect_ident(&mut self) -> Identifier {
        if let Some(id) = self.parse_ident() {
            id
        } else {
            self.error(format!("expected identifier, got {:?}", self.cur().kind));
            Identifier::new("<error>", self.cur_span())
        }
    }

    // ── Annotation ────────────────────────────────────────────────────────────

    pub(super) fn parse_annotations(&mut self) -> Vec<Annotation> {
        let mut anns = Vec::new();
        while self.at(TokenKind::At) {
            let start = self.cur().offset;
            self.advance(); // @
            let mut name = vec![self.expect_ident()];
            while self.eat(TokenKind::Dot).is_some() {
                name.push(self.expect_ident());
            }
            let constructor_name =
                if self.eat(TokenKind::Dot).is_some() { Some(self.expect_ident()) } else { None };
            let args = if self.at(TokenKind::LParen) {
                Some(self.parse_arg_list())
            } else {
                None
            };
            let span = self.span_from(start);
            anns.push(Annotation { name, constructor_name, args, span });
        }
        anns
    }

    // ── Function body ─────────────────────────────────────────────────────────

    pub(super) fn parse_function_body(&mut self) -> Option<FunctionBody> {
        match self.cur().kind {
            TokenKind::Arrow => {
                let start = self.cur().offset;
                self.advance();
                let expr = self.parse_expr();
                self.eat(TokenKind::Semicolon);
                Some(FunctionBody::Arrow(Box::new(expr), self.span_from(start)))
            }
            TokenKind::LBrace => Some(FunctionBody::Block(self.parse_block())),
            TokenKind::Semicolon => {
                // abstract body
                self.advance();
                None
            }
            _ => None,
        }
    }

    // ── Argument list ─────────────────────────────────────────────────────────

    pub(super) fn parse_arg_list(&mut self) -> ArgList {
        let start = self.cur().offset;
        self.expect(TokenKind::LParen);
        let mut positional = Vec::new();
        let mut named = Vec::new();

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            // Check for trailing comma
            if self.at(TokenKind::Comma) { self.advance(); continue; }

            // Named arg: name: expr
            if (self.is_ident_like() || self.at(TokenKind::Ident))
                && self.peek(1).kind == TokenKind::Colon
            {
                let arg_start = self.cur().offset;
                let name = self.expect_ident();
                self.advance(); // :
                let value = self.parse_expr();
                named.push(NamedArg { name, value, span: self.span_from(arg_start) });
            } else {
                positional.push(self.parse_expr());
            }

            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        ArgList { positional, named, span: self.span_from(start) }
    }

    // ── Type argument list <T, U> ──────────────────────────────────────────────

    pub(super) fn parse_type_args(&mut self) -> Vec<DartType> {
        if !self.at(TokenKind::Lt) {
            return Vec::new();
        }
        self.advance(); // <
        let mut args = Vec::new();
        while !self.at(TokenKind::Gt) && !self.at(TokenKind::Eof) {
            args.push(self.parse_type());
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        // consume > or >> (split)
        // For nested types like Map<String, List<int>>, >> must be split:
        // the inner close mutates >> → > in place but does NOT advance, so the
        // enclosing type-args list sees the remaining > and closes on it.
        if self.at(TokenKind::GtGtGt) {
            self.tokens[self.pos].kind = TokenKind::GtGt;
            // Do NOT advance — leave >> for the next two outer closes.
        } else if self.at(TokenKind::GtGt) {
            self.tokens[self.pos].kind = TokenKind::Gt;
            // Do NOT advance — leave > for the outer close.
        } else {
            self.eat(TokenKind::Gt);
        }
        args
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    pub(super) fn parse_program(&mut self) -> Program {
        let start = 0usize;
        let library_directive = self.try_parse_library_directive();
        let part_of_directive = self.try_parse_part_of();
        let mut part_directives = Vec::new();
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut declarations = Vec::new();

        while !self.at(TokenKind::Eof) {
            // Peek past any annotations
            match self.cur().kind {
                TokenKind::Import => imports.push(self.parse_import()),
                TokenKind::Export => exports.push(self.parse_export()),
                TokenKind::Part => {
                    if let Some(p) = self.try_parse_part() {
                        part_directives.push(p);
                    }
                }
                _ => {
                    if let Some(decl) = self.parse_top_level_decl() {
                        declarations.push(decl);
                    } else {
                        // Recovery: skip one token
                        let span = self.cur_span();
                        self.error(format!("unexpected token {:?}", self.cur().kind));
                        self.advance();
                        declarations.push(TopLevelDecl::Error(ErrorNode {
                            message: "unexpected token".into(),
                            span,
                        }));
                    }
                }
            }
        }

        Program {
            library_directive,
            part_of_directive,
            part_directives,
            imports,
            exports,
            declarations,
            span: self.span_from(start),
        }
    }
}

