//! Dart 3.x recursive-descent parser.

mod decl;
mod expr;
mod pattern;
mod stmt;
mod types;

use falcon_syntax::ast::*;
use falcon_syntax::token::{Token, TokenKind};
use tracing::debug;

use crate::lexer::{Lexer, filter_trivia};

/// Parse Dart 3.x into an AST and recoverable errors on a depth-bounded worker
/// stack, so deeply nested input cannot exhaust the caller's stack.
pub fn parse(source: &str) -> (Program, Vec<ParseError>) {
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(PARSER_STACK_SIZE)
            .spawn_scoped(scope, || parse_inner(source))
        {
            Ok(handle) => handle.join().expect("parser thread panicked"),
            // Spawn can fail under thread/memory exhaustion; parsing on the
            // current stack is still protected by the MAX_PARSE_DEPTH guard,
            // just with less headroom — better than aborting the process.
            Err(_) => parse_inner(source),
        }
    })
}

/// Stack for the parser worker thread. Sized so a descent to the full
/// [`MAX_PARSE_DEPTH`](Parser::MAX_PARSE_DEPTH) ceiling clears with wide margin
/// (that ceiling costs well under 16 MB of native stack).
const PARSER_STACK_SIZE: usize = 256 * 1024 * 1024;

fn parse_inner(source: &str) -> (Program, Vec<ParseError>) {
    debug!("parsing file");
    let raw_tokens = Lexer::new(source).tokenize();
    let tokens = filter_trivia(raw_tokens);
    let mut p = Parser::new(tokens, source);
    let program = p.parse_program();
    let error_count = p.errors.len();
    debug!(errors = error_count, "parse complete");
    (program, p.errors)
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Ctx<'src> {
    pub(super) suppress_conditional_qmark: bool,
    pub(super) pattern_binding: bool,
    /// Bracketed sub-parses clear this latch; body-marker scopes must not restore it.
    pub(super) in_ctor_init_value: bool,
    pub(super) body: AsyncMarker,
    pub(super) ctor_name: Option<&'src str>,
}

impl Default for Ctx<'_> {
    fn default() -> Self {
        Self {
            suppress_conditional_qmark: false,
            pattern_binding: false,
            in_ctor_init_value: false,
            body: AsyncMarker::Sync,
            ctor_name: None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Checkpoint {
    pos: usize,
    errors: usize,
    depth_limit_hit: bool,
}

pub(super) struct Parser<'src> {
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) src: &'src str,
    pub(super) errors: Vec<ParseError>,
    pub(super) ctx: Ctx<'src>,
    /// Undo log for the in-place `>>`/`>>>` → `>` splits that [`parse_type_args`]
    /// performs while closing nested generics. Each entry is `(token index,
    /// original kind)`. A speculative rollback must restore these — see
    /// [`rewind_to`] — otherwise a discarded type parse over a shift-token close
    /// leaves the token permanently narrowed and corrupts the re-parse.
    pub(super) gt_undo: Vec<(usize, TokenKind)>,
    /// Current recursive-descent nesting depth and a latch marking that the
    /// [`MAX_PARSE_DEPTH`](Self::MAX_PARSE_DEPTH) guard has fired. Bounds the
    /// hand-rolled recursion so pathological/generated source (deeply nested
    /// parens, lists, blocks, …) yields a parse error instead of overflowing the
    /// stack and aborting the process.
    pub(super) depth: usize,
    pub(super) depth_limit_hit: bool,
}

impl<'src> Parser<'src> {
    pub(super) fn new(tokens: Vec<Token>, src: &'src str) -> Self {
        Self {
            tokens,
            pos: 0,
            src,
            errors: Vec::new(),
            ctx: Ctx::default(),
            gt_undo: Vec::new(),
            depth: 0,
            depth_limit_hit: false,
        }
    }

    /// Recursion-depth ceiling for the recursive-descent parser. Chosen well
    /// above any realistic source nesting yet low enough that the guard fires
    /// before the native stack is exhausted (the guard is applied at several
    /// points per grammatical nesting level).
    pub(super) const MAX_PARSE_DEPTH: usize = 150;

    #[inline]
    pub(super) fn guarded<T>(
        &mut self,
        on_overflow: impl FnOnce(&mut Self) -> T,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if self.depth >= Self::MAX_PARSE_DEPTH {
            if !self.depth_limit_hit {
                self.depth_limit_hit = true;
                self.error("nesting too deep");
            }
            return on_overflow(self);
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    #[inline]
    pub(super) fn with_ctx<T>(
        &mut self,
        edit: impl FnOnce(&mut Ctx<'src>),
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let saved = self.ctx;
        edit(&mut self.ctx);
        let result = f(self);
        self.ctx = saved;
        result
    }

    #[inline]
    pub(super) fn with_body<T>(
        &mut self,
        marker: AsyncMarker,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let saved = self.ctx.body;
        self.ctx.body = marker;
        let result = f(self);
        self.ctx.body = saved;
        result
    }

    #[inline]
    pub(super) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            errors: self.errors.len(),
            depth_limit_hit: self.depth_limit_hit,
        }
    }

    #[inline]
    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        self.rewind_to(checkpoint.pos);
        self.errors.truncate(checkpoint.errors);
        self.depth_limit_hit = checkpoint.depth_limit_hit;
    }

    #[inline]
    pub(super) fn speculate<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let checkpoint = self.checkpoint();
        let result = f(self);
        if result.is_none() {
            self.restore(checkpoint);
        }
        result
    }

    /// Roll the cursor back to `saved`, restoring any `>>`/`>>>` tokens that were
    /// split at or after that position back to their original kind. Speculative
    /// parses that may have entered [`parse_type_args`] must rewind through this
    /// rather than assigning `self.pos` directly, so a discarded type parse never
    /// leaves a narrowed shift token behind.
    pub(super) fn rewind_to(&mut self, saved: usize) {
        while let Some((idx, _)) = self.gt_undo.last() {
            if *idx < saved {
                break;
            }
            let (idx, kind) = self.gt_undo.pop().unwrap();
            self.tokens[idx].kind = kind;
        }
        self.pos = saved;
    }

    /// Consume a trailing `?` as a nullable-type suffix, unless we are parsing an
    /// `is`/`as` type and the `?` begins a conditional expression instead.
    pub(super) fn eat_type_qmark(&mut self) -> bool {
        if !self.at(TokenKind::Qmark) {
            return false;
        }
        if self.ctx.suppress_conditional_qmark && self.token_starts_expr(&self.peek(1).kind) {
            // `T? Function(...)` / `T? Function<...>` is an unambiguous nullable
            // function type after an `is`/`as` type, not a conditional `?` — so the
            // `?` is still a nullable suffix here despite `Function` starting an expr.
            let is_fn_type = self.peek(1).kind == TokenKind::Function
                && matches!(self.peek(2).kind, TokenKind::LParen | TokenKind::Lt);
            if !is_fn_type {
                return false;
            }
        }
        self.advance();
        true
    }

    /// True when `kind` can begin an expression — used to tell a conditional `?`
    /// apart from a nullable-type `?` after an `is`/`as` type.
    pub(super) fn token_starts_expr(&self, kind: &TokenKind) -> bool {
        use TokenKind::*;
        matches!(
            kind,
            IntLit
                | DoubleLit
                | StringLit
                | This
                | Super
                | New
                | Const
                | Switch
                | Throw
                | Await
                | Rethrow
                | True
                | False
                | Null
                | Bang
                | Minus
                | Tilde
                | PlusPlus
                | MinusMinus
                | LParen
                | LBracket
                | LBrace
                // A `<` after an `is`/`as` type begins a typed collection literal
                // in the then-branch of a conditional (`x is T ? <int>[] : y`), so
                // it starts an expression rather than continuing the type.
                | Lt
        ) || kind.is_identifier_like()
    }

    pub(super) fn cur(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or_else(|| self.tokens.last().unwrap())
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
        let tok = *self.cur();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    pub(super) fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) {
            Some(self.advance())
        } else {
            None
        }
    }

    pub(super) fn expect(&mut self, kind: TokenKind) -> Token {
        if self.at(kind) {
            self.advance()
        } else {
            self.error(format!("expected {:?}, got {:?}", kind, self.cur().kind));
            *self.cur()
        }
    }

    pub(super) fn error(&mut self, msg: impl Into<String>) {
        let offset = self.cur().offset;
        self.errors.push(ParseError {
            message: msg.into(),
            offset,
        });
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

    pub(super) fn tok_text(&self, tok: &Token) -> &'src str {
        &self.src[tok.offset..tok.offset + tok.len]
    }

    pub(super) fn cur_text(&self) -> &str {
        self.tok_text(self.cur())
    }

    pub(super) fn tok_ident(&self, tok: &Token) -> Identifier {
        Identifier::new(self.tok_text(tok), Self::tok_span(tok))
    }

    pub(super) fn kind_at(&self, index: usize) -> TokenKind {
        self.tokens
            .get(index)
            .map_or(TokenKind::Eof, |token| token.kind)
    }

    pub(super) fn try_parse_type_followed_by(
        &mut self,
        follows: fn(TokenKind) -> bool,
    ) -> Option<DartType> {
        if !self.is_type_start() {
            return None;
        }
        self.speculate(|parser| {
            let ty = parser.parse_type();
            follows(parser.cur().kind).then_some(ty)
        })
    }

    pub(super) fn var_keyword(is_const: bool, is_final: bool) -> VarKeyword {
        if is_const {
            VarKeyword::Const
        } else if is_final {
            VarKeyword::Final
        } else {
            VarKeyword::Var
        }
    }

    pub(super) fn parse_declarators(&mut self, first: Identifier) -> Vec<VarDeclarator> {
        let mut vars = Vec::new();
        let mut name = first;
        loop {
            let start = name.span.start;
            let initializer = self.eat(TokenKind::Eq).map(|_| self.parse_expr());
            vars.push(VarDeclarator {
                name,
                initializer,
                span: self.span_from(start),
            });
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
            name = self.expect_ident();
        }
        vars
    }

    pub(super) fn parse_ident(&mut self) -> Option<Identifier> {
        if self.cur().kind.is_identifier_like() {
            let tok = self.advance();
            Some(self.tok_ident(&tok))
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

    pub(super) fn parse_annotations(&mut self) -> Vec<Annotation> {
        let mut anns = Vec::new();
        while self.at(TokenKind::At) {
            let start = self.cur().offset;
            self.advance(); // @
            // A `.new` reference names the unnamed constructor (`@X.new()`); `new`
            // is a keyword token, so accept it in the dotted-name segments here and
            // in the trailing constructor name below.
            let mut name = vec![self.expect_ident()];
            while self.eat(TokenKind::Dot).is_some() {
                name.push(self.expect_ctor_name());
            }
            // Type arguments: `@Native<int Function()>(...)`, `@Foo<int>.named()`.
            let type_args = self.parse_type_args();
            let constructor_name = self.eat(TokenKind::Dot).map(|_| self.expect_ctor_name());
            // A `(` here is the annotation's argument list — but only if it parses
            // cleanly as one. Bare metadata directly followed by a record-type
            // member return (`@override ({int a, int b})? m()`, `@override ()? m()`)
            // would otherwise be misread as a set/map-literal argument or empty
            // args; speculatively parse and roll back so the `(` stays for the
            // return type. A `?` right after the `)` is the giveaway that the parens
            // were a nullable record type, since an argument list is never followed
            // by `?`.
            let args = if self.at(TokenKind::LParen) {
                self.speculate(|parser| {
                    let errors = parser.errors.len();
                    let args = parser.parse_arg_list();
                    (parser.errors.len() == errors && !parser.at(TokenKind::Qmark)).then_some(args)
                })
            } else {
                None
            };
            let span = self.span_from(start);
            anns.push(Annotation {
                name,
                type_args,
                constructor_name,
                args,
                span,
            });
        }
        anns
    }

    pub(super) fn parse_function_body(&mut self, marker: AsyncMarker) -> Option<FunctionBody> {
        self.with_body(marker, |parser| match parser.cur().kind {
            TokenKind::Arrow => {
                let start = parser.cur().offset;
                parser.advance();
                let expr = parser.parse_expr();
                // An `=> expr` declaration body is terminated by `;`; a missing one
                // is a syntax error the SDK reports (EXPECTED_TOKEN), not silent
                // recovery. Function *expressions* (closures) use a separate arrow
                // path in `expr.rs` that carries no terminator.
                parser.expect(TokenKind::Semicolon);
                Some(FunctionBody::Arrow(Box::new(expr), parser.span_from(start)))
            }
            TokenKind::LBrace => Some(FunctionBody::Block(parser.parse_block())),
            TokenKind::Semicolon => {
                parser.advance();
                None
            }
            _ => None,
        })
    }

    pub(super) fn parse_arg_list(&mut self) -> ArgList {
        let start = self.cur().offset;
        self.expect(TokenKind::LParen);
        // Inside an argument list a trailing `{` never opens the constructor body.
        self.ctx.in_ctor_init_value = false;
        let mut positional = Vec::new();
        let mut named = Vec::new();

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Comma) {
                self.advance();
                continue;
            }

            // Named arg: name: expr
            if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon {
                let arg_start = self.cur().offset;
                let name = self.expect_ident();
                self.advance(); // :
                let value = self.parse_expr();
                named.push(NamedArg {
                    name,
                    value,
                    span: self.span_from(arg_start),
                });
            } else {
                positional.push(self.parse_expr());
            }

            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        ArgList {
            positional,
            named,
            span: self.span_from(start),
        }
    }

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
            self.gt_undo.push((self.pos, TokenKind::GtGtGt));
            self.tokens[self.pos].kind = TokenKind::GtGt;
            // Do NOT advance — leave >> for the next two outer closes.
        } else if self.at(TokenKind::GtGt) {
            self.gt_undo.push((self.pos, TokenKind::GtGt));
            self.tokens[self.pos].kind = TokenKind::Gt;
            // Do NOT advance — leave > for the outer close.
        } else {
            self.eat(TokenKind::Gt);
        }
        args
    }

    /// Kind of the first token past any leading annotations, without moving the
    /// cursor. Used to route `@meta import/export/part/library …` directives,
    /// whose keyword is hidden behind metadata the directive parser re-consumes.
    pub(super) fn peek_kind_after_annotations(&self) -> TokenKind {
        let i = self.index_after_annotations(self.pos);
        self.tokens.get(i).map(|t| t.kind).unwrap_or(TokenKind::Eof)
    }

    /// Token index after skipping any run of annotations starting at `i`. A pure
    /// lookahead helper (no cursor mutation); approximate by design — it only has
    /// to land on the token that follows the metadata.
    fn index_after_annotations(&self, mut i: usize) -> usize {
        let n = self.tokens.len();
        while i < n && self.tokens[i].kind == TokenKind::At {
            i += 1; // @
            if i < n && self.tokens[i].kind.is_identifier_like() {
                i += 1; // first name segment
            }
            // dotted name: `.seg` while the next token names a segment (an
            // identifier or `new`, as in `@X.new`)
            while i + 1 < n
                && self.tokens[i].kind == TokenKind::Dot
                && (self.tokens[i + 1].kind.is_identifier_like()
                    || self.tokens[i + 1].kind == TokenKind::New)
            {
                i += 2;
            }
            // type arguments: `@Native<int Function()>(…)`
            if i < n && self.tokens[i].kind == TokenKind::Lt {
                i = self.index_after_balanced_angles(i);
            }
            // constructor name after type args: `@Foo<int>.named(…)`, `@Foo<int>.new(…)`
            if i + 1 < n
                && self.tokens[i].kind == TokenKind::Dot
                && (self.tokens[i + 1].kind.is_identifier_like()
                    || self.tokens[i + 1].kind == TokenKind::New)
            {
                i += 2;
            }
            // argument list `(…)`
            if i < n && self.tokens[i].kind == TokenKind::LParen {
                i = self.index_after_balanced_parens(i);
            }
        }
        i
    }

    /// Index just past a balanced `(…)` starting at `i` (which must be `LParen`).
    fn index_after_balanced_parens(&self, mut i: usize) -> usize {
        let n = self.tokens.len();
        let mut depth = 0i32;
        while i < n {
            match self.tokens[i].kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    i += 1;
                    if depth <= 0 {
                        return i;
                    }
                    continue;
                }
                TokenKind::Eof => return i,
                _ => {}
            }
            i += 1;
        }
        i
    }

    /// Index just past a balanced `<…>` starting at `i` (which must be `Lt`),
    /// accounting for the merged `>>`/`>>>` close tokens.
    fn index_after_balanced_angles(&self, mut i: usize) -> usize {
        let n = self.tokens.len();
        let mut depth = 0i32;
        while i < n {
            match self.tokens[i].kind {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => depth -= 1,
                TokenKind::GtGt => depth -= 2,
                TokenKind::GtGtGt => depth -= 3,
                // A bare `;`/`{`/EOF cannot appear inside real type args — bail so a
                // stray `<` never swallows the rest of the file.
                TokenKind::Semicolon | TokenKind::LBrace | TokenKind::Eof => return i,
                _ => {}
            }
            i += 1;
            if depth <= 0 {
                return i;
            }
        }
        i
    }

    pub(super) fn parse_program(&mut self) -> Program {
        let start = 0usize;
        let library_directive = self.try_parse_library_directive();
        let part_of_directive = self.try_parse_part_of();
        let mut part_directives = Vec::new();
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut declarations = Vec::new();

        while !self.at(TokenKind::Eof) {
            // Route past any leading metadata so `@meta import/export/part …`
            // reaches the directive parser (which re-consumes the annotations).
            match self.peek_kind_after_annotations() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_type_probe_discards_only_speculative_diagnostics() {
        let src = "Map<int, +>;";
        let mut parser = Parser::new(Lexer::new(src).tokenize(), src);
        parser.error("retained");
        assert!(
            parser
                .try_parse_type_followed_by(TokenKind::is_identifier_like)
                .is_none()
        );
        assert_eq!(parser.cur_text(), "Map");
        assert_eq!(parser.errors.len(), 1);
        assert_eq!(parser.errors[0].message, "retained");
        parser.parse_type();
        assert!(parser.errors.len() > 1);
    }

    #[test]
    fn speculative_depth_limit_does_not_suppress_real_error() {
        let mut parser = Parser::new(Lexer::new("x").tokenize(), "x");
        parser.depth = Parser::MAX_PARSE_DEPTH;
        let result: Option<()> = parser.speculate(|parser| {
            parser.parse_expr();
            None
        });
        assert!(result.is_none());
        assert!(parser.errors.is_empty());
        assert!(!parser.depth_limit_hit);
        parser.parse_expr();
        assert_eq!(
            parser
                .errors
                .iter()
                .filter(|error| error.message == "nesting too deep")
                .count(),
            1
        );
        let checkpoint = parser.checkpoint();
        parser.depth_limit_hit = false;
        parser.restore(checkpoint);
        parser.parse_expr();
        assert_eq!(
            parser
                .errors
                .iter()
                .filter(|error| error.message == "nesting too deep")
                .count(),
            1
        );
    }
}
