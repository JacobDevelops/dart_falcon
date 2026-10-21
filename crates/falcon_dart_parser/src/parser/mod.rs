//! Dart 3.x recursive-descent parser.

mod decl;
mod expr;
mod lexical;
mod pattern;
mod stmt;
mod types;

pub use lexical::lex_tree;

use falcon_syntax::LanguageVersion;
use falcon_syntax::cst::{AsyncMarker, Event, NodeKind, Span, SyntaxTree};
use falcon_syntax::token::{Token, TokenKind};
use tracing::debug;

use crate::lexer::Lexer;

/// Parsed concrete syntax and recoverable diagnostics.
#[derive(Debug)]
pub struct ParseResult {
    pub errors: Vec<ParseError>,
    pub tree: SyntaxTree,
}

/// Parse Dart into a CST and recoverable errors at the language version of a
/// file outside any package: its `// @dart` override, else
/// [`LanguageVersion::CURRENT`].
///
/// # Panics
///
/// Panics if `source` exceeds the compact token limit of `u32::MAX` bytes.
pub fn parse(source: &str) -> ParseResult {
    parse_for_package(source, None)
}

/// Parse Dart into a CST and recoverable errors at the file's effective
/// language version — its `// @dart` override, else `package_version`, else
/// [`LanguageVersion::CURRENT`] — on a depth-bounded worker stack, so deeply
/// nested input cannot exhaust the caller's stack. A worker that cannot start
/// yields an empty program with one error; use [`try_parse_for_package`] to
/// tell that apart from a real parse.
///
/// # Panics
///
/// Panics if `source` exceeds the compact token limit of `u32::MAX` bytes.
pub fn parse_for_package(source: &str, package_version: Option<LanguageVersion>) -> ParseResult {
    try_parse_for_package(source, package_version).unwrap_or_else(|error| {
        let tokens = Lexer::new(source).tokenize();
        let tree = SyntaxTree::build(
            source,
            &lexical::cst_tokens(&tokens, source, &[]),
            Vec::new(),
        );
        ParseResult {
            errors: vec![ParseError {
                kind: ParseErrorKind::Syntax,
                message: format!("could not start parser thread: {error}"),
                offset: 0,
            }],
            tree,
        }
    })
}

/// [`parse_for_package`], failing when the parser worker thread cannot start.
///
/// # Errors
///
/// Returns the spawn error when the worker thread cannot be created.
///
/// # Panics
///
/// Panics if `source` exceeds the compact token limit of `u32::MAX` bytes.
pub fn try_parse_for_package(
    source: &str,
    package_version: Option<LanguageVersion>,
) -> std::io::Result<ParseResult> {
    let version = LanguageVersion::file_override(source)
        .or(package_version)
        .unwrap_or(LanguageVersion::CURRENT);
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(PARSER_STACK_SIZE)
            .spawn_scoped(scope, || parse_inner(source, version))?;
        Ok(handle
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload)))
    })
}

/// Stack for the parser worker thread. Sized so a descent to the full
/// [`MAX_PARSE_DEPTH`](Parser::MAX_PARSE_DEPTH) ceiling clears with wide margin
/// (that ceiling costs well under 16 MB of native stack).
const PARSER_STACK_SIZE: usize = 256 * 1024 * 1024;

fn parse_inner(source: &str, version: LanguageVersion) -> ParseResult {
    debug!("parsing file");
    let tokens = Lexer::new(source).tokenize();
    let mut p = Parser::new(&tokens, source, version);
    p.parse_program();
    p.interpolation_tokens
        .sort_unstable_by_key(|(span, _)| span.start);
    let tree = SyntaxTree::build(
        source,
        &lexical::cst_tokens(&tokens, source, &p.interpolation_tokens),
        p.events,
    );
    debug_assert!(
        tree.is_lossless(source),
        "CST must retain every source byte"
    );
    let errors = p.errors;
    debug!(errors = errors.len(), "parse complete");
    ParseResult { errors, tree }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Syntax,
    FeatureGate,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub message: String,
    pub offset: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Ctx<'src> {
    pub(super) suppress_conditional_qmark: bool,
    pub(super) pattern_binding: bool,
    /// Bracketed sub-parses temporarily clear this latch.
    pub(super) in_ctor_init_value: bool,
    /// The enclosing switch guard's top-level arrow is an arm separator, not a lambda.
    pub(super) switch_guard_arrow: Option<usize>,
    pub(super) body: AsyncMarker,
    pub(super) ctor_name: Option<&'src str>,
}

impl Default for Ctx<'_> {
    fn default() -> Self {
        Self {
            suppress_conditional_qmark: false,
            pattern_binding: false,
            in_ctor_init_value: false,
            switch_guard_arrow: None,
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
    events: usize,
    forward_parents: usize,
    interpolation_tokens: usize,
    split: Option<Token>,
    split_count: usize,
    in_ctor_init_value: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Marker {
    pos: usize,
}

#[derive(Clone, Copy)]
pub(super) struct CompletedMarker {
    pos: usize,
    kind: NodeKind,
    span: Span,
}

impl Marker {
    pub(super) fn complete(self, parser: &mut Parser<'_>, kind: NodeKind) -> CompletedMarker {
        let Event::Start {
            kind: slot, offset, ..
        } = &mut parser.events[self.pos]
        else {
            panic!("marker must refer to an open node");
        };
        *slot = kind;
        let offset = *offset;
        parser.events.push(Event::Finish);
        let span = match (
            parser.events.get(self.pos + 1),
            parser.events.get(parser.events.len() - 2),
        ) {
            (Some(Event::RecoverySpan(span)), _) | (_, Some(Event::RecoverySpan(span))) => *span,
            _ => parser.span_from(offset),
        };
        CompletedMarker {
            pos: self.pos,
            kind,
            span,
        }
    }

    pub(super) fn abandon(self, parser: &mut Parser<'_>) {
        parser.events[self.pos] = Event::Tombstone;
    }
}

impl CompletedMarker {
    pub(super) fn text<'src>(self, parser: &Parser<'src>) -> &'src str {
        if self.kind == NodeKind::Identifier
            && matches!(
                parser.events.get(self.pos + 1),
                Some(Event::RecoverySpan(_))
            )
        {
            "<error>"
        } else {
            &parser.src[self.span.start..self.span.end]
        }
    }

    pub(super) fn precede(self, parser: &mut Parser<'_>) -> Marker {
        let Event::Start { offset, .. } = parser.events[self.pos] else {
            panic!("completed marker must refer to a node");
        };
        let marker = parser.start_at(offset);
        let Event::Start { forward_parent, .. } = &mut parser.events[self.pos] else {
            panic!("completed marker must refer to a node");
        };
        parser.forward_parents.push((self.pos, *forward_parent));
        *forward_parent = Some(marker.pos - self.pos);
        marker
    }
}

/// Temporary dense indices borrowing canonical tokens, not copying them.
pub(super) struct SignificantTokens<'a> {
    tokens: &'a [Token],
    indices: Vec<u32>,
}

impl<'a> SignificantTokens<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        let indices = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| !token.is_trivia())
            .map(|(index, _)| u32::try_from(index).expect("lexer exceeds u32 token capacity"))
            .collect();
        Self { tokens, indices }
    }

    fn len(&self) -> usize {
        self.indices.len()
    }

    fn get(&self, index: usize) -> Option<&Token> {
        self.indices
            .get(index)
            .map(|index| &self.tokens[*index as usize])
    }

    fn last(&self) -> Option<&Token> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    /// Slice before mapping so lookahead starts in O(1), independent of the prefix.
    fn iter_from(
        &self,
        start: usize,
    ) -> Option<impl ExactSizeIterator<Item = &Token> + DoubleEndedIterator> {
        Some(
            self.indices
                .get(start..)?
                .iter()
                .map(|index| &self.tokens[*index as usize]),
        )
    }
}

impl std::ops::Index<usize> for SignificantTokens<'_> {
    type Output = Token;

    fn index(&self, index: usize) -> &Token {
        self.get(index)
            .expect("significant token index out of bounds")
    }
}

pub(super) struct Parser<'src> {
    pub(super) events: Vec<Event>,
    forward_parents: Vec<(usize, Option<usize>)>,
    interpolation_tokens: Vec<(Span, Vec<Token>)>,
    pub(super) tokens: SignificantTokens<'src>,
    /// Lazily paired significant-token brackets for initializer-free statement lookahead.
    pub(super) statement_delimiters: Option<Vec<u32>>,
    pub(super) pos: usize,
    pub(super) src: &'src str,
    /// The language version the source is parsed at; gates grammar that
    /// changed meaning between versions.
    pub(super) version: LanguageVersion,
    pub(super) errors: Vec<ParseError>,
    pub(super) ctx: Ctx<'src>,
    /// A narrowed view of the current shift token while closing generic types.
    split: Option<Token>,
    split_count: usize,
    /// Current recursive-descent nesting depth and a latch marking that the
    /// [`MAX_PARSE_DEPTH`](Self::MAX_PARSE_DEPTH) guard has fired. Bounds the
    /// hand-rolled recursion so pathological/generated source (deeply nested
    /// parens, lists, blocks, …) yields a parse error instead of overflowing the
    /// stack and aborting the process.
    pub(super) depth: usize,
    pub(super) depth_limit_hit: bool,
    /// Cursor checkpoint at the `Function` keyword of the most recently
    /// completed function type; after a type parse, its outermost function type.
    pub(super) last_function_keyword: Option<Checkpoint>,
}

impl<'src> Parser<'src> {
    pub(super) fn start(&mut self) -> Marker {
        self.start_at(self.cur().offset() + self.split_count)
    }

    pub(super) fn start_at(&mut self, offset: usize) -> Marker {
        let marker = Marker {
            pos: self.events.len(),
        };
        self.events.push(Event::Start {
            kind: NodeKind::Error,
            forward_parent: None,
            offset,
        });
        marker
    }

    pub(super) fn missing(&mut self, kind: NodeKind) -> CompletedMarker {
        let marker = self.start();
        if kind == NodeKind::Identifier {
            self.events.push(Event::RecoverySpan(self.cur_span()));
        }
        marker.complete(self, kind)
    }

    pub(super) fn new(tokens: &'src [Token], src: &'src str, version: LanguageVersion) -> Self {
        Self {
            events: Vec::new(),
            forward_parents: Vec::new(),
            interpolation_tokens: Vec::new(),
            tokens: SignificantTokens::new(tokens),
            statement_delimiters: None,
            pos: 0,
            src,
            version,
            errors: Vec::new(),
            ctx: Ctx::default(),
            split: None,
            split_count: 0,
            depth: 0,
            depth_limit_hit: false,
            last_function_keyword: None,
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
            events: self.events.len(),
            forward_parents: self.forward_parents.len(),
            interpolation_tokens: self.interpolation_tokens.len(),
            split: self.split,
            split_count: self.split_count,
            in_ctor_init_value: self.ctx.in_ctor_init_value,
        }
    }

    #[inline]
    pub(super) fn restore(&mut self, checkpoint: Checkpoint) {
        self.restore_cursor(checkpoint);
        self.errors.truncate(checkpoint.errors);
        self.depth_limit_hit = checkpoint.depth_limit_hit;
    }

    #[inline]
    pub(super) fn restore_cursor(&mut self, checkpoint: Checkpoint) {
        self.pos = checkpoint.pos;
        while self.forward_parents.len() > checkpoint.forward_parents {
            let (pos, previous) = self.forward_parents.pop().unwrap();
            if let Event::Start { forward_parent, .. } = &mut self.events[pos] {
                *forward_parent = previous;
            }
        }
        self.events.truncate(checkpoint.events);
        self.interpolation_tokens
            .truncate(checkpoint.interpolation_tokens);
        self.split = checkpoint.split;
        self.split_count = checkpoint.split_count;
        self.ctx.in_ctor_init_value = checkpoint.in_ctor_init_value;
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
        self.split.as_ref().unwrap_or_else(|| {
            self.tokens
                .get(self.pos)
                .unwrap_or_else(|| self.tokens.last().unwrap())
        })
    }

    pub(super) fn peek(&self, offset: usize) -> &Token {
        if offset == 0 {
            return self.cur();
        }
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
        self.events.push(Event::Token {
            kind: tok.kind,
            offset: tok.offset() + self.split_count,
            len: tok.len().saturating_sub(self.split_count),
        });
        self.split = None;
        self.split_count = 0;
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
        self.error_at(self.cur().offset(), msg);
    }

    /// Report a diagnostic at a retained token or node span after consuming it.
    pub(super) fn error_at(&mut self, offset: usize, msg: impl Into<String>) {
        self.errors.push(ParseError {
            kind: ParseErrorKind::Syntax,
            message: msg.into(),
            offset,
        });
    }

    pub(super) fn feature_error(&mut self, msg: impl Into<String>) {
        self.feature_error_at(self.cur().offset(), msg);
    }

    pub(super) fn feature_error_at(&mut self, offset: usize, msg: impl Into<String>) {
        self.errors.push(ParseError {
            kind: ParseErrorKind::FeatureGate,
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
        let end = if self.split.is_some() {
            self.cur().offset() + self.split_count
        } else {
            self.pos.checked_sub(1).map_or(start, |index| {
                let token = &self.tokens[index];
                token.offset() + token.len()
            })
        };
        Span::new(start, end.max(start))
    }

    pub(super) fn cur_span(&self) -> Span {
        let t = self.cur();
        Span::new(t.offset(), t.offset() + t.len())
    }

    pub(super) fn cur_text(&self) -> &str {
        self.cur().text(self.src)
    }

    pub(super) fn kind_at(&self, index: usize) -> TokenKind {
        self.tokens
            .get(index)
            .map_or(TokenKind::Eof, |token| token.kind)
    }

    pub(super) fn try_parse_type_followed_by(
        &mut self,
        follows: fn(TokenKind) -> bool,
    ) -> Option<CompletedMarker> {
        if !self.is_type_start() {
            return None;
        }
        self.speculate(|parser| {
            let ty = parser.parse_type();
            follows(parser.cur().kind).then_some(ty)
        })
    }

    pub(super) fn parse_declarators(&mut self, first: CompletedMarker) {
        let mut name = first;
        loop {
            let marker = name.precede(self);
            if self.eat(TokenKind::Eq).is_some() {
                self.parse_expr();
            }
            marker.complete(self, NodeKind::VarDeclarator);
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
            name = self.expect_ident();
        }
    }

    pub(super) fn parse_ident(&mut self) -> Option<CompletedMarker> {
        if self.cur().kind.is_identifier_like() {
            let marker = self.start();
            self.advance();
            Some(marker.complete(self, NodeKind::Identifier))
        } else {
            None
        }
    }

    pub(super) fn expect_ident(&mut self) -> CompletedMarker {
        if let Some(id) = self.parse_ident() {
            id
        } else {
            self.error(format!("expected identifier, got {:?}", self.cur().kind));
            self.missing(NodeKind::Identifier)
        }
    }

    pub(super) fn parse_annotations(&mut self) {
        while self.at(TokenKind::At) {
            let marker = self.start();
            self.advance(); // @
            // A `.new` reference names the unnamed constructor (`@X.new()`); `new`
            // is a keyword token, so accept it in the dotted-name segments here and
            // in the trailing constructor name below.
            self.expect_ident();
            while self.eat(TokenKind::Dot).is_some() {
                self.expect_ctor_name();
            }
            // Type arguments: `@Native<int Function()>(...)`, `@Foo<int>.named()`.
            self.parse_type_args();
            if self.eat(TokenKind::Dot).is_some() {
                self.expect_ctor_name();
            }
            // A `(` here is the annotation's argument list — but only if it parses
            // cleanly as one. Bare metadata directly followed by a record-type
            // member return (`@override ({int a, int b})? m()`, `@override ()? m()`)
            // would otherwise be misread as a set/map-literal argument or empty
            // args; speculatively parse and roll back so the `(` stays for the
            // return type. A `?` right after the `)` is the giveaway that the parens
            // were a nullable record type, since an argument list is never followed
            // by `?`.
            if self.at(TokenKind::LParen) {
                self.speculate(|parser| {
                    let errors = parser.errors.len();
                    parser.parse_arg_list();
                    (parser.errors.len() == errors && !parser.at(TokenKind::Qmark)).then_some(())
                });
            }
            marker.complete(self, NodeKind::Annotation);
        }
    }

    pub(super) fn parse_function_body(&mut self, marker: AsyncMarker) -> Option<CompletedMarker> {
        self.with_body(marker, |parser| match parser.cur().kind {
            TokenKind::Arrow => {
                let node = parser.start();
                parser.advance();
                parser.parse_expr();
                // An `=> expr` declaration body is terminated by `;`; a missing one
                // is a syntax error the SDK reports (EXPECTED_TOKEN), not silent
                // recovery. Function *expressions* (closures) use a separate arrow
                // path in `expr.rs` that carries no terminator.
                parser.expect(TokenKind::Semicolon);
                Some(node.complete(parser, NodeKind::ArrowBody))
            }
            TokenKind::LBrace => Some(parser.parse_block()),
            TokenKind::Semicolon => {
                parser.advance();
                None
            }
            // SDK-internal `native` bodies: `int get length native;`,
            // `void f() native "name";`.
            TokenKind::Ident if parser.cur_text() == "native" => {
                let node = parser.start();
                parser.advance();
                if parser.at(TokenKind::StringLit) {
                    parser.parse_adjacent_strings();
                }
                let body = node.complete(parser, NodeKind::NativeBody);
                parser.expect(TokenKind::Semicolon);
                Some(body)
            }
            _ => None,
        })
    }

    pub(super) fn parse_arg_list(&mut self) -> CompletedMarker {
        self.with_ctx(
            |ctx| ctx.in_ctor_init_value = false,
            Self::parse_arg_list_inner,
        )
    }

    fn parse_arg_list_inner(&mut self) -> CompletedMarker {
        let marker = self.start();
        self.expect(TokenKind::LParen);

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Comma) {
                self.advance();
                continue;
            }

            // Named arg: name: expr
            if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon {
                let named_marker = self.start();
                self.expect_ident();
                self.advance(); // :
                self.parse_expr();
                named_marker.complete(self, NodeKind::NamedArg);
            } else {
                self.parse_expr();
            }

            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        marker.complete(self, NodeKind::ArgList)
    }

    pub(super) fn parse_type_args(&mut self) -> usize {
        if !self.at(TokenKind::Lt) {
            return 0;
        }
        let marker = self.start();
        self.advance(); // <
        let mut count = 0;
        while !self.at(TokenKind::Gt) && !self.at(TokenKind::Eof) {
            self.parse_type();
            count += 1;
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        // Narrow only the cursor view; canonical shift tokens stay unchanged.
        let narrowed = match self.cur().kind {
            TokenKind::GtGtGt => Some(TokenKind::GtGt),
            TokenKind::GtGt => Some(TokenKind::Gt),
            _ => None,
        };
        if let Some(kind) = narrowed {
            self.events.push(Event::Token {
                kind: TokenKind::Gt,
                offset: self.cur().offset() + self.split_count,
                len: 1,
            });
            self.split = Some(Token::new(kind, self.cur().offset(), self.cur().len()));
            self.split_count += 1;
        } else {
            self.eat(TokenKind::Gt);
        }
        marker.complete(self, NodeKind::TypeArgs);
        count
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

    pub(super) fn parse_program(&mut self) -> CompletedMarker {
        let marker = self.start();
        let mut allow_library = true;
        let mut allow_part_of = true;

        while !self.at(TokenKind::Eof) {
            let item = self.start();
            self.parse_annotations();
            let kind = self.cur().kind;
            if allow_library && kind == TokenKind::Library {
                self.parse_library_directive(item);
                allow_library = false;
                continue;
            }
            allow_library = false;
            if allow_part_of
                && kind == TokenKind::Part
                && self.peek(1).kind == TokenKind::Ident
                && self.peek(1).text(self.src) == "of"
            {
                self.parse_part_of(item);
                allow_part_of = false;
                continue;
            }
            allow_part_of = false;
            match kind {
                TokenKind::Import => {
                    self.parse_import(item);
                }
                TokenKind::Export => {
                    self.parse_export(item);
                }
                TokenKind::Part => {
                    self.parse_part(item);
                }
                _ => {
                    if let Some(kind) = self.parse_top_level_decl() {
                        item.complete(self, kind);
                    } else {
                        let recovery = self.start();
                        self.missing(NodeKind::Identifier);
                        recovery.complete(self, NodeKind::Error);
                        self.error(format!("unexpected token {:?}", self.cur().kind));
                        if !self.at(TokenKind::Eof) {
                            self.advance();
                        }
                        item.complete(self, NodeKind::ErrorNode);
                    }
                }
            }
        }

        marker.complete(self, NodeKind::Program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn significant_iteration_starts_directly_at_each_index() {
        let source = "/* trivia */ first + second < third > (fourth);";
        let syntax = Lexer::new(source).tokenize();
        let tokens = SignificantTokens::new(&syntax);
        for (start, canonical) in tokens.indices.iter().enumerate() {
            let mut suffix = tokens.iter_from(start).unwrap();
            assert_eq!(suffix.len(), tokens.len() - start);
            assert!(std::ptr::eq(
                suffix.next().unwrap(),
                &syntax[*canonical as usize]
            ));
            assert_eq!(suffix.len(), tokens.len() - start - 1);
        }
        assert_eq!(tokens.iter_from(tokens.len()).unwrap().len(), 0);
        assert!(tokens.iter_from(tokens.len() + 1).is_none());
        assert!(tokens.iter_from(usize::MAX).is_none());

        let parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
        assert_eq!(
            parser.matching_close(usize::MAX, expr::Brackets::Mixed, |_| false, None),
            None
        );
        assert_eq!(
            parser.matching_angle_close(usize::MAX, |_| true, None),
            None
        );
    }

    #[test]
    fn generic_cursor_splits_leave_retained_shift_tokens_unchanged() {
        let source = "Map<int, List<int>> value;";
        let syntax = Lexer::new(source).tokenize();
        let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
        let checkpoint = parser.checkpoint();
        parser.parse_type();
        assert!(parser.errors.is_empty());
        assert_eq!(parser.cur_text(), "value");
        assert!(syntax.iter().any(|token| token.kind == TokenKind::GtGt));
        parser.restore(checkpoint);
        parser.parse_type();
        assert!(parser.errors.is_empty());
        assert_eq!(parser.cur_text(), "value");
    }

    #[test]
    fn retained_trivia_and_significant_cursor_share_canonical_tokens() {
        let source = "// doc\nvar value = 1;";
        let syntax = Lexer::new(source).tokenize();
        let parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
        assert_eq!(parser.cur().kind, TokenKind::Var);
        let index = parser.tokens.indices[0] as usize;
        assert!(std::ptr::eq(parser.cur(), &syntax[index]));
        assert_eq!(
            syntax
                .iter()
                .filter(|token| token.kind == TokenKind::LineComment)
                .count(),
            1
        );
        assert_eq!(syntax[0].kind, TokenKind::LineComment);
        let reconstructed: String = syntax.iter().map(|token| token.text(source)).collect();
        assert_eq!(reconstructed, source);
    }

    #[test]
    fn failed_type_probe_discards_only_speculative_diagnostics() {
        let src = "Map<int, +>;";
        let syntax = Lexer::new(src).tokenize();
        let mut parser = Parser::new(&syntax, src, LanguageVersion::CURRENT);
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
        let syntax = Lexer::new("x").tokenize();
        let mut parser = Parser::new(&syntax, "x", LanguageVersion::CURRENT);
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

    #[test]
    fn additive_chain_event_count() {
        let source = format!("var value = {}a;", "a + ".repeat(200_000));
        let tokens = Lexer::new(&source).tokenize();
        let mut parser = Parser::new(&tokens, &source, LanguageVersion::CURRENT);
        parser.parse_program();
        assert!(parser.errors.is_empty());
        eprintln!("{} bytes: {} events", source.len(), parser.events.len());
        assert!(parser.events.len() < 1_300_000);
    }

    #[test]
    fn completed_markers_distinguish_recovery_spans_from_missing_names() {
        let source = "name next";
        let tokens = Lexer::new(source).tokenize();
        let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
        let field = parser.start();
        parser.expect_ident();
        let recovery = Span::new(0, 4);
        parser.events.push(Event::RecoverySpan(recovery));
        let field = field.complete(&mut parser, NodeKind::ObjectPatternField);
        assert_eq!(field.span, recovery);
        assert_eq!(field.text(&parser), "name");
        let missing = parser.missing(NodeKind::Identifier);
        assert_eq!(missing.span, Span::new(5, 9));
        assert_eq!(missing.text(&parser), "<error>");
    }

    #[test]
    fn cst_round_trips_recovery_trivia_generics_and_strings() {
        for source in [
            "// comment\n/// docs\nclass C { /* member */ int x = 1; }\n",
            "Map<String, List<List<int>>> value;",
            "void f() { print('hello $name ${call(\"nested ${value}\")}'); }",
            "var text = 'unterminated",
            "@@@ } ??? \u{2603}",
        ] {
            let result = parse(source);
            let text: String = result
                .tree
                .tokens(source)
                .map(|token| token.text())
                .collect();
            assert_eq!(text, source);
            assert_eq!(result.tree.root(source).kind(), NodeKind::Program);
        }
    }

    #[test]
    fn markers_wrap_abandon_and_restore_forward_parents() {
        let source = "a + b";
        let syntax = Lexer::new(source).tokenize();
        let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
        let root = parser.start();
        let left = parser.start();
        parser.advance();
        let left = left.complete(&mut parser, NodeKind::Identifier);
        let checkpoint = parser.checkpoint();
        let discarded = left.precede(&mut parser);
        parser.advance();
        discarded.complete(&mut parser, NodeKind::BinaryExpr);
        parser.restore(checkpoint);
        assert_eq!(parser.events.len(), checkpoint.events);
        let wrapper = left.precede(&mut parser);
        parser.advance();
        let abandoned = parser.start();
        abandoned.abandon(&mut parser);
        let right = parser.start();
        parser.advance();
        right.complete(&mut parser, NodeKind::Identifier);
        let wrapped = wrapper.complete(&mut parser, NodeKind::BinaryExpr);
        assert_eq!(wrapped.span, Span::new(0, source.len()));
        assert_eq!(wrapped.text(&parser), source);
        root.complete(&mut parser, NodeKind::Program);
        let tree = SyntaxTree::build(source, &syntax, parser.events);
        let binary = tree.root(source).children().next().unwrap();
        assert_eq!(binary.kind(), NodeKind::BinaryExpr);
        assert_eq!(binary.children().count(), 2);
        assert_eq!(binary.text(), source);
    }

    #[test]
    fn cst_recovery_leaves_and_program_spans() {
        for source in [
            "",
            "// comment only",
            " \n// header\nvar x = 1; // tail",
            "\u{feff}",
            "\u{feff}// comment",
            "\u{feff}#!/usr/bin/dart\nvar x = 1;",
        ] {
            let result = parse(source);
            let end = Lexer::new(source)
                .tokenize()
                .into_iter()
                .rfind(|token| !token.is_trivia() && token.kind != TokenKind::Eof)
                .map_or(0, |token| token.offset() + token.len());
            assert_eq!(result.tree.program(source).span(), Span::new(0, end));
            assert!(result.tree.is_lossless(source));
        }
        let source = "class C { int x = 1; int y; int get size native; }";
        let result = parse(source);
        let root = result.tree.root(source);
        let names: Vec<_> = root
            .descendants()
            .filter(|node| node.kind() == NodeKind::VarDeclarator)
            .map(|node| node.significant_tokens().next().unwrap().text())
            .collect();
        assert_eq!(names, ["x", "y"]);
        let native = root
            .descendants()
            .find(|node| node.kind() == NodeKind::NativeBody)
            .unwrap();
        assert_eq!(native.text(), "native");
        let source = "@;";
        let result = parse(source);
        let missing = result
            .tree
            .root(source)
            .descendants()
            .find(|node| node.kind() == NodeKind::Identifier)
            .unwrap();
        let name = falcon_syntax::cst::Identifier::from_node(missing).unwrap();
        assert_eq!(name.name(), "<error>");
        assert_eq!(name.span(), Span::new(1, 2));
        let source = "var x = '${foo(}';";
        let result = parse(source);
        assert!(!result.errors.is_empty());
        assert!(
            !result
                .tree
                .root(source)
                .descendants()
                .any(|node| node.kind() == NodeKind::StringInterpolation)
        );
    }

    #[test]
    fn missing_name_preserves_original_string_and_split_token_spans() {
        for (source, expected) in [
            ("'$x'", Span::new(0, 4)),
            (">>", Span::new(0, 2)),
            ("", Span::new(0, 0)),
        ] {
            let syntax = Lexer::new(source).tokenize();
            let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
            let root = parser.start();
            if source == ">>" {
                parser.split = Some(*parser.cur());
                parser.split_count = 1;
            }
            parser.expect_ident();
            while !parser.at(TokenKind::Eof) {
                parser.advance();
            }
            root.complete(&mut parser, NodeKind::Program);
            let tokens = lexical::cst_tokens(&syntax, source, &[]);
            let tree = SyntaxTree::build(source, &tokens, parser.events);
            let node = tree
                .root(source)
                .descendants()
                .find(|node| node.kind() == NodeKind::Identifier)
                .unwrap();
            let identifier = falcon_syntax::cst::Identifier::from_node(node).unwrap();
            assert!(identifier.is_missing());
            assert_eq!(identifier.span(), expected);
            assert_eq!(identifier.name(), "<error>");
            assert_eq!(node.span().start, node.span().end);
        }
    }

    #[test]
    fn missing_identifier_recovery_after_declarations_and_before_interpolation() {
        for source in ["var x = 1; var", "void f() { a.'$x'; }"] {
            let result = parse(source);
            assert!(result.tree.is_lossless(source));
            assert!(
                result
                    .tree
                    .root(source)
                    .descendants()
                    .filter(|node| node.kind() == NodeKind::Identifier)
                    .filter_map(falcon_syntax::cst::Identifier::from_node)
                    .all(|identifier| identifier.span().end <= source.len())
            );
        }
    }

    #[test]
    fn cursor_restore_preserves_diagnostics_and_depth_latch() {
        let source = "x";
        let syntax = Lexer::new(source).tokenize();
        let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
        let checkpoint = parser.checkpoint();
        parser.advance();
        parser.error("retained recovery diagnostic");
        parser.depth_limit_hit = true;
        parser.restore_cursor(checkpoint);
        assert_eq!(parser.cur_text(), "x");
        assert!(parser.events.is_empty());
        assert_eq!(parser.errors.len(), 1);
        assert!(parser.depth_limit_hit);
        parser.restore(checkpoint);
        assert!(parser.errors.is_empty());
        assert!(!parser.depth_limit_hit);
    }

    #[test]
    fn checkpoint_restores_constructor_initializer_latch() {
        let source = "(value) {}";
        let tokens = Lexer::new(source).tokenize();
        let mut parser = Parser::new(&tokens, source, LanguageVersion::CURRENT);
        parser.ctx.in_ctor_init_value = true;
        let checkpoint = parser.checkpoint();
        parser.parse_expr();
        assert!(parser.ctx.in_ctor_init_value);
        parser.ctx.in_ctor_init_value = false;
        parser.restore_cursor(checkpoint);
        assert!(parser.ctx.in_ctor_init_value);
        parser.parse_expr();
        assert_eq!(parser.cur().kind, TokenKind::LBrace);
        assert!(parser.errors.is_empty(), "{:?}", parser.errors);
    }
}
