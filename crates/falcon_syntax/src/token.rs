use crate::ast::Span;
use std::sync::OnceLock;

/// Store-local lexical ID: canonical tokens first, then simple interpolation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenId(pub u32);

impl TokenId {
    pub fn index(self) -> usize {
        self.0 as usize
    }

    fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("syntax store exceeds u32 token capacity"))
    }

    fn validate_count(count: usize) {
        if let Some(last_index) = count.checked_sub(1) {
            Self::from_index(last_index);
        }
    }
}

/// Half-open range of store-local lexical IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenRange {
    pub start: TokenId,
    pub end: TokenId,
}

/// Borrowed canonical documentation tokens attached to the next nontrivia token.
#[derive(Debug, Clone, Copy)]
pub struct DocBlock<'a> {
    pub start: usize,
    /// Source offset of the first nontrivia token after the block, including EOF.
    pub next_token: usize,
    tokens: &'a [Token],
}

impl<'a> DocBlock<'a> {
    pub fn tokens(&self) -> impl Iterator<Item = &'a Token> {
        self.tokens
            .iter()
            .filter(|token| token.kind == TokenKind::DocComment)
    }
}

/// Retained lexical data. Source text remains owned by the caller.
#[derive(Debug)]
pub struct SyntaxStore {
    tokens: Box<[Token]>,
    interpolations: Box<[(Span, SyntaxStore)]>,
    simple_interpolations: Box<[Token]>,
    delimiter_pairs: OnceLock<Box<[u32]>>,
}

impl Clone for SyntaxStore {
    fn clone(&self) -> Self {
        fn clone_local(store: &SyntaxStore) -> SyntaxStore {
            SyntaxStore {
                tokens: store.tokens.clone(),
                interpolations: Box::new([]),
                simple_interpolations: store.simple_interpolations.clone(),
                delimiter_pairs: OnceLock::new(),
            }
        }

        let mut cloned = clone_local(self);
        if self.interpolations.is_empty() {
            return cloned;
        }
        // Leaf children need no worklist entry, so shallow stores allocate only
        // their cloned data. Deeper trees reuse this worklist during traversal.
        let mut pending = Vec::new();
        let mut current = Some((self, &mut cloned));
        while let Some((source, target)) = current {
            target.interpolations = source
                .interpolations
                .iter()
                .map(|(span, store)| (*span, clone_local(store)))
                .collect();
            for ((_, source), (_, target)) in source
                .interpolations
                .iter()
                .zip(target.interpolations.iter_mut())
            {
                if !source.interpolations.is_empty() {
                    pending.push((source, target));
                }
            }
            current = pending.pop();
        }
        cloned
    }
}

impl Drop for SyntaxStore {
    fn drop(&mut self) {
        if self.interpolations.is_empty() {
            return;
        }
        let mut children = std::mem::take(&mut self.interpolations);
        let mut pending = Vec::new();
        loop {
            for (_, mut child) in children.into_vec() {
                if !child.interpolations.is_empty() {
                    pending.push(std::mem::take(&mut child.interpolations));
                }
            }
            let Some(next) = pending.pop() else {
                break;
            };
            children = next;
        }
    }
}

impl SyntaxStore {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens: tokens.into_boxed_slice(),
            interpolations: Box::new([]),
            simple_interpolations: Box::new([]),
            delimiter_pairs: OnceLock::new(),
        }
    }

    pub fn with_interpolations(mut self, mut interpolations: Vec<(Span, SyntaxStore)>) -> Self {
        interpolations.sort_unstable_by_key(|(span, _)| span.start);
        self.interpolations = interpolations.into_boxed_slice();
        self
    }

    pub fn with_simple_interpolations(mut self, mut tokens: Vec<Token>) -> Self {
        let total = self
            .tokens
            .len()
            .checked_add(tokens.len())
            .expect("syntax store exceeds token capacity");
        TokenId::validate_count(total);
        tokens.sort_unstable_by_key(|token| token.offset());
        self.simple_interpolations = tokens.into_boxed_slice();
        self
    }

    pub fn interpolation(&self, span: Span) -> Option<&Self> {
        let index = self
            .interpolations
            .binary_search_by_key(&span.start, |(range, _)| range.start)
            .ok()?;
        let (range, store) = &self.interpolations[index];
        (*range == span).then_some(store)
    }

    pub fn interpolation_end(&self, start: usize) -> Option<usize> {
        let mut current = self;
        loop {
            let index = current
                .interpolations
                .partition_point(|(span, _)| span.start <= start);
            let (span, store) = &current.interpolations[index.checked_sub(1)?];
            if span.start == start {
                return Some(span.end);
            }
            if start >= span.end {
                return None;
            }
            current = store;
        }
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn token(&self, id: TokenId) -> Option<&Token> {
        let index = id.index();
        self.tokens.get(index).or_else(|| {
            index
                .checked_sub(self.tokens.len())
                .and_then(|index| self.simple_interpolations.get(index))
        })
    }

    /// Resolve fully contained tokens, using appended IDs for simple interpolation names.
    pub fn token_range(&self, span: Span) -> TokenRange {
        let start = self
            .tokens
            .partition_point(|token| token.offset() < span.start);
        let end = self.tokens[start..].partition_point(|token| {
            token.offset() < span.end
                && !token.is_empty()
                && token.offset() + token.len() <= span.end
        });
        if end == 0 {
            let simple_start = self
                .simple_interpolations
                .partition_point(|token| token.offset() < span.start);
            let simple_end = self.simple_interpolations[simple_start..].partition_point(|token| {
                token.offset() < span.end
                    && !token.is_empty()
                    && token.offset() + token.len() <= span.end
            });
            if simple_end != 0 {
                return TokenRange {
                    start: TokenId::from_index(self.tokens.len() + simple_start),
                    end: TokenId::from_index(self.tokens.len() + simple_start + simple_end),
                };
            }
        }
        TokenRange {
            start: TokenId::from_index(start),
            end: TokenId::from_index(start + end),
        }
    }

    /// Tokens fully contained in a source range. A range inside an interpolation
    /// uses its retained stream; a complete string remains a single token.
    pub fn tokens_in(&self, span: Span) -> std::slice::Iter<'_, Token> {
        let store = self.syntax_for_span(span);
        let range = store.token_range(span);
        if range.start.index() >= store.tokens.len() && range.start != range.end {
            store.simple_interpolations
                [range.start.index() - store.tokens.len()..range.end.index() - store.tokens.len()]
                .iter()
        } else {
            store.tokens[range.start.index()..range.end.index()].iter()
        }
    }

    /// Resolve the store whose local token IDs describe this source range.
    pub fn syntax_for_span(&self, span: Span) -> &Self {
        let mut current = self;
        loop {
            let index = current
                .interpolations
                .partition_point(|(inner, _)| inner.start <= span.start);
            let Some((_, store)) = index
                .checked_sub(1)
                .and_then(|index| current.interpolations.get(index))
                .filter(|(inner, _)| span.end <= inner.end)
            else {
                return current;
            };
            current = store;
        }
    }

    /// Direct braced interpolation interiors contained by a range in its local store.
    pub fn interpolation_ranges(&self, span: Span) -> impl Iterator<Item = Span> {
        let store = self.syntax_for_span(span);
        let start = store
            .interpolations
            .partition_point(|(inner, _)| inner.start < span.start);
        store.interpolations[start..]
            .iter()
            .take_while(move |(inner, _)| inner.end <= span.end)
            .map(|(inner, _)| *inner)
    }

    pub fn significant_tokens(&self) -> impl Iterator<Item = &Token> {
        self.expanded_tokens()
            .filter(|token| !token.is_trivia() && token.kind != TokenKind::Eof)
    }

    pub fn comment_tokens(&self) -> impl Iterator<Item = &Token> {
        self.expanded_tokens()
            .filter(|token| token.kind.is_comment())
    }

    /// Root identifiers and interpolation identifier-like names, excluding root keywords.
    pub fn identifier_tokens(&self) -> impl Iterator<Item = &Token> {
        let mut tokens = self.expanded_tokens();
        tokens.identifiers_only = true;
        tokens
    }

    pub fn keywords_in(&self, span: Span, kind: TokenKind) -> impl Iterator<Item = &Token> {
        self.tokens_in(span).filter(move |token| token.kind == kind)
    }

    pub fn comments_in(&self, span: Span) -> impl Iterator<Item = &Token> {
        self.tokens_in(span).filter(|token| token.kind.is_comment())
    }

    /// Documentation before each canonical code token, including EOF.
    /// Plain comments are skipped; `/**` discards earlier docs in the same gap,
    /// while subsequent `///` comments remain part of that block.
    pub fn doc_blocks<'a>(&'a self, source: &'a str) -> impl Iterator<Item = DocBlock<'a>> + 'a {
        let mut tokens = self.tokens.iter().enumerate();
        std::iter::from_fn(move || {
            let mut start = None;
            for (index, token) in tokens.by_ref() {
                if token.kind == TokenKind::DocComment {
                    if start.is_none() || token.text(source).starts_with("/**") {
                        start = Some(index);
                    }
                } else if !token.is_trivia()
                    && let Some(start) = start.take()
                {
                    return Some(DocBlock {
                        start: self.tokens[start].offset(),
                        next_token: token.offset(),
                        tokens: &self.tokens[start..index],
                    });
                }
            }
            None
        })
    }

    /// Start of the last attached documentation group in a declaration's trivia gap.
    pub fn documentation_start(&self, gap: Span, source: &str) -> Option<usize> {
        let mut start = None;
        let mut previous_block = false;
        for token in self
            .tokens_in(gap)
            .filter(|token| token.kind == TokenKind::DocComment)
        {
            let is_block = token.text(source).starts_with("/**");
            if is_block || previous_block || start.is_none() {
                start = Some(token.offset());
            }
            previous_block = is_block;
        }
        start
    }

    /// Find the corresponding closing delimiter after a canonical opening token.
    pub fn closing_delimiter(&self, open: TokenId) -> Option<TokenId> {
        matches!(
            self.token(open)?.kind,
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace | TokenKind::QmarkLBracket
        )
        .then(|| self.paired_delimiter(open))
        .flatten()
    }

    /// Find the corresponding opening delimiter before a canonical closing token.
    pub fn opening_delimiter(&self, close: TokenId) -> Option<TokenId> {
        matches!(
            self.token(close)?.kind,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace
        )
        .then(|| self.paired_delimiter(close))
        .flatten()
    }

    fn paired_delimiter(&self, token: TokenId) -> Option<TokenId> {
        let pairs = self.delimiter_pairs.get_or_init(|| {
            let mut pairs = vec![u32::MAX; self.tokens.len()];
            let mut stacks = [u32::MAX; 3];
            for (index, token) in self.tokens.iter().enumerate() {
                let (stack, opening) = match token.kind {
                    TokenKind::LParen => (0, true),
                    TokenKind::LBracket | TokenKind::QmarkLBracket => (1, true),
                    TokenKind::LBrace => (2, true),
                    TokenKind::RParen => (0, false),
                    TokenKind::RBracket => (1, false),
                    TokenKind::RBrace => (2, false),
                    _ => continue,
                };
                if opening {
                    // Unmatched openings temporarily link to the previous opening of their kind.
                    pairs[index] = stacks[stack];
                    stacks[stack] = TokenId::from_index(index).0;
                } else if stacks[stack] != u32::MAX {
                    let open = stacks[stack] as usize;
                    stacks[stack] = pairs[open];
                    pairs[open] = TokenId::from_index(index).0;
                    pairs[index] = TokenId::from_index(open).0;
                }
            }
            for mut open in stacks {
                while open != u32::MAX {
                    let previous = pairs[open as usize];
                    pairs[open as usize] = u32::MAX;
                    open = previous;
                }
            }
            pairs.into_boxed_slice()
        });
        let paired = *pairs.get(token.index())?;
        (paired != u32::MAX).then_some(TokenId(paired))
    }

    fn expanded_tokens(&self) -> ExpandedTokens<'_> {
        ExpandedTokens {
            root: TokenFrame::new(self),
            stack: Vec::new(),
            identifiers_only: false,
        }
    }
}

struct TokenFrame<'a> {
    store: &'a SyntaxStore,
    canonical: usize,
    simple: usize,
    interpolation: usize,
}

impl<'a> TokenFrame<'a> {
    fn new(store: &'a SyntaxStore) -> Self {
        Self {
            store,
            canonical: 0,
            simple: 0,
            interpolation: 0,
        }
    }
}

struct ExpandedTokens<'a> {
    root: TokenFrame<'a>,
    stack: Vec<TokenFrame<'a>>,
    identifiers_only: bool,
}

impl<'a> Iterator for ExpandedTokens<'a> {
    type Item = &'a Token;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let in_interpolation = !self.stack.is_empty();
            let frame = self.stack.last_mut().unwrap_or(&mut self.root);
            let canonical = frame.store.tokens.get(frame.canonical);
            let simple = self
                .identifiers_only
                .then(|| frame.store.simple_interpolations.get(frame.simple))
                .flatten();
            let use_simple = simple.is_some_and(|simple| {
                canonical.is_none_or(|canonical| simple.offset() < canonical.offset())
            });
            let token = if use_simple { simple } else { canonical };
            if let Some((span, child)) = frame.store.interpolations.get(frame.interpolation)
                && token.is_none_or(|token| span.start <= token.offset())
            {
                frame.interpolation += 1;
                self.stack.push(TokenFrame::new(child));
                continue;
            }
            let Some(token) = token else {
                self.stack.pop()?;
                continue;
            };
            if use_simple {
                frame.simple += 1;
            } else {
                frame.canonical += 1;
            }
            if self.identifiers_only
                && token.kind != TokenKind::Ident
                && !((in_interpolation || use_simple) && token.kind.is_identifier_like())
            {
                continue;
            }
            return Some(token);
        }
    }
}

/// A nine-byte lexical token with compact source coordinates copied by accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C, packed)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte offset of the first character in the source string.
    offset: u32,
    /// Byte length of the token in the source string.
    len: u32,
}

impl Token {
    /// Construct a token with byte coordinates.
    ///
    /// # Panics
    ///
    /// Panics if either coordinate exceeds `u32::MAX`. Sources are limited to
    /// `u32::MAX` bytes, matching the syntax store's checked token-ID capacity.
    pub fn new(kind: TokenKind, offset: usize, len: usize) -> Self {
        Self {
            kind,
            offset: u32::try_from(offset).expect("token offset exceeds u32 source capacity"),
            len: u32::try_from(len).expect("token length exceeds u32 source capacity"),
        }
    }

    pub fn offset(&self) -> usize {
        self.offset as usize
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.offset()..self.offset() + self.len()]
    }

    pub fn is_trivia(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::DocComment
        )
    }
}

/// Dart 3.x token kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TokenKind {
    IntLit,
    DoubleLit,
    StringLit,

    Ident,

    Assert,
    Break,
    Case,
    Catch,
    Class,
    Const,
    Continue,
    Default,
    Do,
    Else,
    Enum,
    Extends,
    False,
    Final,
    Finally,
    For,
    If,
    In,
    Is,
    New,
    Null,
    Rethrow,
    Return,
    Super,
    Switch,
    This,
    Throw,
    True,
    Try,
    Var,
    Void,
    While,
    With,

    Abstract,
    As,
    Base,
    Covariant,
    Deferred,
    Dynamic,
    Export,
    Extension,
    External,
    Factory,
    Function,
    Get,
    Hide,
    Implements,
    Import,
    Interface,
    Late,
    Library,
    Mixin,
    Operator,
    Part,
    Required,
    Sealed,
    Set,
    Show,
    Static,
    Type,
    Typedef,

    Async,
    Await,
    Sync,
    Yield,

    When,

    On,       // `on` in mixin on-clause and try/catch
    Override, // `override` built-in identifier

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    TildeSlash, // ~/

    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,

    AmpAmp,
    PipePipe,
    Bang,

    Amp,
    Pipe,
    Caret,
    Tilde,
    LtLt,
    GtGt,
    GtGtGt,

    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    TildeSlashEq,
    AmpEq,
    PipeEq,
    CaretEq,
    LtLtEq,
    GtGtEq,
    GtGtGtEq,
    QmarkQmarkEq, // ??=

    Qmark,         // ?
    QmarkQmark,    // ??
    QmarkDot,      // ?.
    QmarkLBracket, // ?[

    PlusPlus,
    MinusMinus,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    At,
    Hash, // #  (symbol-literal introducer)

    Dot,
    DotDot,         // ..  cascade
    DotDotQmark,    // ?.. null-safe cascade
    DotDotDot,      // ... spread
    DotDotDotQmark, // ...? null-aware spread

    Arrow, // =>

    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    DocComment,

    Eof,
    Error,
}

impl TokenKind {
    pub fn is_comment(self) -> bool {
        matches!(
            self,
            Self::LineComment | Self::BlockComment | Self::DocComment
        )
    }

    pub fn is_identifier_like(self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Ident
                | Abstract
                | As
                | Base
                | Covariant
                | Deferred
                | Dynamic
                | Export
                | Extension
                | External
                | Factory
                | Function
                | Get
                | Hide
                | Implements
                | Import
                | Interface
                | Late
                | Library
                | Mixin
                | Operator
                | Part
                | Required
                | Sealed
                | Set
                | Show
                | Static
                | Type
                | Typedef
                | Async
                | Await
                | Sync
                | Yield
                | When
                | On
                | Override
        )
    }

    /// Single-token operators; `[]` and `[]=` require parsing multiple tokens.
    pub fn is_user_definable_operator(self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Lt | Gt
                | LtEq
                | GtEq
                | EqEq
                | Minus
                | Plus
                | Slash
                | TildeSlash
                | Star
                | Percent
                | Pipe
                | Caret
                | Amp
                | LtLt
                | GtGt
                | GtGtGt
                | Tilde
        )
    }

    /// Returns the keyword kind for the given identifier text, if it is a keyword.
    pub fn from_keyword(text: &str) -> Option<TokenKind> {
        let kind = match text {
            "assert" => TokenKind::Assert,
            "break" => TokenKind::Break,
            "case" => TokenKind::Case,
            "catch" => TokenKind::Catch,
            "class" => TokenKind::Class,
            "const" => TokenKind::Const,
            "continue" => TokenKind::Continue,
            "default" => TokenKind::Default,
            "do" => TokenKind::Do,
            "else" => TokenKind::Else,
            "enum" => TokenKind::Enum,
            "extends" => TokenKind::Extends,
            "false" => TokenKind::False,
            "final" => TokenKind::Final,
            "finally" => TokenKind::Finally,
            "for" => TokenKind::For,
            "if" => TokenKind::If,
            "in" => TokenKind::In,
            "is" => TokenKind::Is,
            "new" => TokenKind::New,
            "null" => TokenKind::Null,
            "rethrow" => TokenKind::Rethrow,
            "return" => TokenKind::Return,
            "super" => TokenKind::Super,
            "switch" => TokenKind::Switch,
            "this" => TokenKind::This,
            "throw" => TokenKind::Throw,
            "true" => TokenKind::True,
            "try" => TokenKind::Try,
            "var" => TokenKind::Var,
            "void" => TokenKind::Void,
            "while" => TokenKind::While,
            "with" => TokenKind::With,
            // Built-in identifiers
            "abstract" => TokenKind::Abstract,
            "as" => TokenKind::As,
            "base" => TokenKind::Base,
            "covariant" => TokenKind::Covariant,
            "deferred" => TokenKind::Deferred,
            "dynamic" => TokenKind::Dynamic,
            "export" => TokenKind::Export,
            "extension" => TokenKind::Extension,
            "external" => TokenKind::External,
            "factory" => TokenKind::Factory,
            "Function" => TokenKind::Function,
            "get" => TokenKind::Get,
            "hide" => TokenKind::Hide,
            "implements" => TokenKind::Implements,
            "import" => TokenKind::Import,
            "interface" => TokenKind::Interface,
            "late" => TokenKind::Late,
            "library" => TokenKind::Library,
            "mixin" => TokenKind::Mixin,
            "operator" => TokenKind::Operator,
            "part" => TokenKind::Part,
            "required" => TokenKind::Required,
            "sealed" => TokenKind::Sealed,
            "set" => TokenKind::Set,
            "show" => TokenKind::Show,
            "static" => TokenKind::Static,
            "type" => TokenKind::Type,
            "typedef" => TokenKind::Typedef,
            // Async / generator
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "sync" => TokenKind::Sync,
            "yield" => TokenKind::Yield,
            // Dart 3.x
            "when" => TokenKind::When,
            // Contextual
            "on" => TokenKind::On,
            "override" => TokenKind::Override,
            _ => return None,
        };
        Some(kind)
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;

    #[test]
    fn deep_store_clone_drop_and_queries_fit_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                const DEPTH: usize = 16_384;
                let mut store = SyntaxStore::new(vec![Token::new(TokenKind::Ident, DEPTH, 1)])
                    .with_simple_interpolations(vec![Token::new(TokenKind::Ident, DEPTH + 1, 1)]);
                // Construct O(depth) retained data directly, without repeatedly scanning
                // nested source literals or retaining a quadratic-sized source fixture.
                for start in (0..DEPTH).rev() {
                    let end = 2 * DEPTH + 2 - start;
                    store = SyntaxStore::new(vec![Token::new(
                        TokenKind::StringLit,
                        start,
                        end - start,
                    )])
                    .with_interpolations(vec![(Span::new(start, end), store)]);
                }
                let cloned = store.clone();
                for tree in [&store, &cloned] {
                    assert_eq!(tree.interpolation_end(0), Some(2 * DEPTH + 2));
                    assert_eq!(tree.interpolation_end(DEPTH - 1), Some(DEPTH + 3));
                    assert_eq!(tree.interpolation_end(DEPTH), None);
                    assert_eq!(tree.tokens_in(Span::new(DEPTH, DEPTH + 1)).count(), 1);
                    let local = tree.syntax_for_span(Span::new(DEPTH, DEPTH + 2));
                    assert_eq!(local.tokens()[0], Token::new(TokenKind::Ident, DEPTH, 1));
                    assert_eq!(
                        tree.tokens_in(Span::new(DEPTH + 1, DEPTH + 2))
                            .copied()
                            .next(),
                        Some(Token::new(TokenKind::Ident, DEPTH + 1, 1)),
                    );
                }
                drop(store);
                drop(cloned);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn retained_token_layout_preserves_offsets_and_compact_ids() {
        assert_eq!(std::mem::size_of::<Token>(), 9);
        assert_eq!(std::mem::align_of::<Token>(), 1);
        assert_eq!(std::mem::size_of::<TokenKind>(), 1);
        assert_eq!(std::mem::size_of::<TokenId>(), 4);
        assert_eq!(std::mem::size_of::<TokenRange>(), 8);
    }

    #[test]
    fn compact_token_coordinates_preserve_u32_boundaries() {
        let end = Token::new(TokenKind::Eof, u32::MAX as usize, 0);
        assert_eq!(end.offset(), u32::MAX as usize);
        assert_eq!(end.len(), 0);
        assert!(end.is_empty());
        let full = Token::new(TokenKind::StringLit, 0, u32::MAX as usize);
        assert_eq!(full.offset(), 0);
        assert_eq!(full.len(), u32::MAX as usize);
        assert!(!full.is_empty());
    }

    #[test]
    fn token_capacity_accepts_empty_and_single_token_stores() {
        TokenId::validate_count(0);
        TokenId::validate_count(1);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn token_capacity_preserves_full_u32_source_and_eof() {
        // One token per source byte plus EOF still has a representable final ID.
        let count = u32::MAX as usize + 1;
        TokenId::validate_count(count);
        assert_eq!(TokenId::from_index(count - 1), TokenId(u32::MAX));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "syntax store exceeds u32 token capacity")]
    fn token_capacity_rejects_unrepresentable_final_id() {
        TokenId::validate_count(u32::MAX as usize + 2);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "token offset exceeds u32 source capacity")]
    fn compact_token_rejects_offset_overflow() {
        Token::new(TokenKind::Eof, u32::MAX as usize + 1, 0);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "token length exceeds u32 source capacity")]
    fn compact_token_rejects_length_overflow() {
        Token::new(TokenKind::StringLit, 0, u32::MAX as usize + 1);
    }

    #[test]
    fn doc_blocks_borrow_canonical_tokens_and_preserve_grouping_through_eof() {
        let parts = [
            (TokenKind::DocComment, "/// first"),
            (TokenKind::Newline, "\n"),
            (TokenKind::LineComment, "// plain"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/// second"),
            (TokenKind::Newline, "\n"),
            (TokenKind::Ident, "x"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/// discarded"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/** also discarded */"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/** reset */"),
            (TokenKind::Newline, "\n"),
            (TokenKind::BlockComment, "/* skipped */"),
            (TokenKind::Whitespace, " "),
            (TokenKind::DocComment, "/// appended"),
            (TokenKind::Newline, "\n"),
            (TokenKind::Ident, "y"),
            (TokenKind::Newline, "\n"),
            (TokenKind::DocComment, "/// eof"),
            (TokenKind::Newline, "\n"),
        ];
        let source = parts.iter().map(|(_, text)| *text).collect::<String>();
        let mut offset = 0;
        let mut tokens = parts
            .iter()
            .map(|(kind, text)| {
                let token = Token::new(*kind, offset, text.len());
                offset += text.len();
                token
            })
            .collect::<Vec<_>>();
        tokens.push(Token::new(TokenKind::Eof, source.len(), 0));
        let store = SyntaxStore::new(tokens);
        let mut blocks = store.doc_blocks(&source);
        for (indices, next) in [(&[0, 4][..], 6), (&[12, 16][..], 18), (&[20][..], 22)] {
            let block = blocks.next().unwrap();
            assert_eq!(block.start, store.tokens()[indices[0]].offset());
            assert_eq!(block.next_token, store.tokens()[next].offset());
            let mut docs = block.tokens();
            for &index in indices {
                assert!(std::ptr::eq(docs.next().unwrap(), &store.tokens()[index]));
            }
            assert!(docs.next().is_none());
        }
        assert!(blocks.next().is_none());
        assert!(blocks.next().is_none());
    }

    #[test]
    fn doc_blocks_do_not_flush_unterminated_trivia_without_a_code_token() {
        let source = "/// trailing";
        let store = SyntaxStore::new(vec![Token::new(TokenKind::DocComment, 0, source.len())]);
        assert!(store.doc_blocks(source).next().is_none());
    }

    #[test]
    fn doc_blocks_exclude_retained_interpolation_comments() {
        let source = "'${/// hidden\nx}'";
        let start = source.find("///").unwrap();
        let end = source.find('}').unwrap();
        let inner = SyntaxStore::new(vec![
            Token::new(TokenKind::DocComment, start, "/// hidden".len()),
            Token::new(TokenKind::Ident, end - 1, 1),
            Token::new(TokenKind::Eof, end, 0),
        ]);
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::StringLit, 0, source.len()),
            Token::new(TokenKind::Eof, source.len(), 0),
        ])
        .with_interpolations(vec![(Span::new(start, end), inner)]);
        assert!(store.doc_blocks(source).next().is_none());
    }

    #[test]
    fn delimiter_and_documentation_helpers_use_canonical_trivia() {
        let source = "/** a */\n/// b\n(x(y))";
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::DocComment, 0, 8),
            Token::new(TokenKind::Newline, 8, 1),
            Token::new(TokenKind::DocComment, 9, 5),
            Token::new(TokenKind::Newline, 14, 1),
            Token::new(TokenKind::LParen, 15, 1),
            Token::new(TokenKind::Ident, 16, 1),
            Token::new(TokenKind::LParen, 17, 1),
            Token::new(TokenKind::Ident, 18, 1),
            Token::new(TokenKind::RParen, 19, 1),
            Token::new(TokenKind::RParen, 20, 1),
        ]);
        assert_eq!(store.documentation_start(Span::new(0, 15), source), Some(9));
        assert_eq!(store.comments_in(Span::new(0, 15)).count(), 2);
        assert_eq!(store.closing_delimiter(TokenId(4)), Some(TokenId(9)));
        assert_eq!(store.closing_delimiter(TokenId(6)), Some(TokenId(8)));
        assert_eq!(store.opening_delimiter(TokenId(9)), Some(TokenId(4)));
        assert_eq!(store.opening_delimiter(TokenId(8)), Some(TokenId(6)));
        assert_eq!(store.closing_delimiter(TokenId(5)), None);
        assert_eq!(store.opening_delimiter(TokenId(4)), None);
        let cloned = store.clone();
        assert!(cloned.delimiter_pairs.get().is_none());
        assert_eq!(cloned.closing_delimiter(TokenId(4)), Some(TokenId(9)));
    }

    #[test]
    fn delimiter_pairs_handle_mixed_nullable_and_unclosed_delimiters() {
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::LParen, 0, 1),
            Token::new(TokenKind::QmarkLBracket, 1, 2),
            Token::new(TokenKind::LBrace, 3, 1),
            Token::new(TokenKind::RBrace, 4, 1),
            Token::new(TokenKind::RBracket, 5, 1),
            Token::new(TokenKind::RParen, 6, 1),
            Token::new(TokenKind::LParen, 7, 1),
            Token::new(TokenKind::LParen, 8, 1),
        ]);
        for (open, close) in [(0, 5), (1, 4), (2, 3)] {
            assert_eq!(store.closing_delimiter(TokenId(open)), Some(TokenId(close)));
            assert_eq!(store.opening_delimiter(TokenId(close)), Some(TokenId(open)));
        }
        assert_eq!(store.closing_delimiter(TokenId(6)), None);
        assert_eq!(store.closing_delimiter(TokenId(7)), None);
        assert_eq!(store.closing_delimiter(TokenId(8)), None);
    }

    #[test]
    fn ranges_exclude_partial_tokens_and_preserve_stable_ids() {
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::Ident, 0, 3),
            Token::new(TokenKind::Whitespace, 3, 1),
            Token::new(TokenKind::Ident, 4, 2),
            Token::new(TokenKind::Eof, 6, 0),
        ]);
        let range = store.token_range(Span::new(1, 5));
        assert_eq!(
            range,
            TokenRange {
                start: TokenId(1),
                end: TokenId(2)
            }
        );
        assert_eq!(store.tokens_in(Span::new(1, 5)).count(), 1);
        assert_eq!(store.token(TokenId(2)).unwrap().offset(), 4);
        assert_eq!(
            store
                .significant_tokens()
                .map(Token::offset)
                .collect::<Vec<_>>(),
            [0, 4]
        );
    }

    #[test]
    fn expression_ranges_use_retained_interpolation_without_expanding_whole_literals() {
        let inner = SyntaxStore::new(vec![
            Token::new(TokenKind::Ident, 3, 1),
            Token::new(TokenKind::Eof, 4, 0),
        ]);
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::StringLit, 0, 6),
            Token::new(TokenKind::Eof, 6, 0),
        ])
        .with_interpolations(vec![(Span::new(3, 4), inner)])
        .with_simple_interpolations(vec![Token::new(TokenKind::Ident, 8, 4)]);
        assert_eq!(
            store.tokens_in(Span::new(3, 4)).next().unwrap().kind,
            TokenKind::Ident
        );
        assert_eq!(
            store.tokens_in(Span::new(0, 6)).next().unwrap().kind,
            TokenKind::StringLit
        );
        assert_eq!(
            store
                .significant_tokens()
                .map(|token| token.kind)
                .collect::<Vec<_>>(),
            vec![TokenKind::StringLit, TokenKind::Ident]
        );
        assert_eq!(
            store
                .identifier_tokens()
                .map(|token| token.offset())
                .collect::<Vec<_>>(),
            vec![3, 8]
        );
        assert_eq!(store.interpolation_end(3), Some(4));
        let inner_store = store.syntax_for_span(Span::new(3, 4));
        let range = inner_store.token_range(Span::new(3, 4));
        assert_eq!(
            inner_store.token(range.start).unwrap().kind,
            TokenKind::Ident
        );
    }

    #[test]
    fn simple_interpolation_ranges_resolve_appended_ids_without_expanding_strings() {
        let source = "'$foo $bar'";
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::StringLit, 0, source.len()),
            Token::new(TokenKind::Eof, source.len(), 0),
        ])
        .with_simple_interpolations(vec![
            Token::new(TokenKind::Ident, 7, 3),
            Token::new(TokenKind::Ident, 2, 3),
        ]);
        for (span, expected) in [(Span::new(2, 5), "foo"), (Span::new(7, 10), "bar")] {
            let local = store.syntax_for_span(span);
            let range = local.token_range(span);
            assert!(range.start.index() >= local.tokens().len());
            assert_eq!(range.end.index() - range.start.index(), 1);
            assert_eq!(local.token(range.start).unwrap().text(source), expected);
            assert_eq!(store.tokens_in(span).next().unwrap().text(source), expected);
        }
        assert_eq!(
            store
                .tokens_in(Span::new(0, source.len()))
                .next()
                .unwrap()
                .kind,
            TokenKind::StringLit
        );
        assert_eq!(store.closing_delimiter(TokenId(2)), None);
    }

    #[test]
    fn identifiers_merge_simple_and_braced_names_in_source_order() {
        let nested = SyntaxStore::new(vec![
            Token::new(TokenKind::StringLit, 6, 8),
            Token::new(TokenKind::Show, 16, 4),
        ])
        .with_simple_interpolations(vec![Token::new(TokenKind::Ident, 8, 4)]);
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::StringLit, 0, 30),
            Token::new(TokenKind::Ident, 31, 5),
        ])
        .with_interpolations(vec![(Span::new(6, 21), nested)])
        .with_simple_interpolations(vec![
            Token::new(TokenKind::Ident, 2, 3),
            Token::new(TokenKind::Ident, 24, 3),
        ]);
        assert_eq!(
            store
                .identifier_tokens()
                .map(|token| token.offset())
                .collect::<Vec<_>>(),
            [2, 8, 16, 24, 31]
        );
        assert_eq!(store.interpolation_end(6), Some(21));
        assert_eq!(store.interpolation_end(22), None);
        assert!(store.interpolation(Span::new(6, 20)).is_none());
        assert!(store.interpolation(Span::new(6, 21)).is_some());
    }

    #[test]
    fn nested_terminal_expression_ranges_exclude_eof_but_keep_canonical_eof() {
        let expression = SyntaxStore::new(vec![
            Token::new(TokenKind::Ident, 8, 1),
            Token::new(TokenKind::Whitespace, 9, 1),
            Token::new(TokenKind::EqEq, 10, 2),
            Token::new(TokenKind::Whitespace, 12, 1),
            Token::new(TokenKind::Ident, 13, 1),
            Token::new(TokenKind::Eof, 14, 0),
        ]);
        let nested = SyntaxStore::new(vec![Token::new(TokenKind::StringLit, 5, 11)])
            .with_interpolations(vec![(Span::new(8, 14), expression)]);
        let store = SyntaxStore::new(vec![Token::new(TokenKind::StringLit, 0, 19)])
            .with_interpolations(vec![(Span::new(3, 17), nested)]);
        let left = store
            .tokens_in(Span::new(8, 9))
            .map(|token| token.kind)
            .collect::<Vec<_>>();
        let right = store
            .tokens_in(Span::new(13, 14))
            .map(|token| token.kind)
            .collect::<Vec<_>>();
        assert_eq!(left, [TokenKind::Ident]);
        assert_eq!(left, right);
        let local = store.syntax_for_span(Span::new(13, 14));
        assert_eq!(local.tokens().last().unwrap().kind, TokenKind::Eof);
        let range = local.token_range(Span::new(13, 14));
        assert_eq!(range.end.index() - range.start.index(), 1);
        assert_eq!(
            store
                .interpolation_ranges(Span::new(0, 19))
                .collect::<Vec<_>>(),
            [Span::new(3, 17)]
        );
        assert_eq!(
            store
                .interpolation_ranges(Span::new(5, 16))
                .collect::<Vec<_>>(),
            [Span::new(8, 14)]
        );
    }

    #[test]
    fn identifier_queries_exclude_root_keywords_but_keep_interpolation_contextual_names() {
        let expression = SyntaxStore::new(vec![
            Token::new(TokenKind::Required, 20, 8),
            Token::new(TokenKind::Eof, 28, 0),
        ]);
        let store = SyntaxStore::new(vec![
            Token::new(TokenKind::Required, 0, 8),
            Token::new(TokenKind::Show, 9, 4),
            Token::new(TokenKind::StringLit, 16, 14),
            Token::new(TokenKind::Ident, 31, 4),
        ])
        .with_interpolations(vec![(Span::new(20, 28), expression)]);
        assert_eq!(
            store
                .identifier_tokens()
                .map(|token| token.offset())
                .collect::<Vec<_>>(),
            [20, 31]
        );
    }
}
