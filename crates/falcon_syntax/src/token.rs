/// A lexical token produced by the Dart lexer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte offset of the first character in the source string.
    pub offset: usize,
    /// Byte length of the token in the source string.
    pub len: usize,
}

impl Token {
    pub fn new(kind: TokenKind, offset: usize, len: usize) -> Self {
        Self { kind, offset, len }
    }

    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.offset..self.offset + self.len]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ── Literals ──────────────────────────────────────────────────────────────
    IntLit,
    DoubleLit,
    StringLit,

    // ── Identifier ────────────────────────────────────────────────────────────
    Ident,

    // ── Reserved words ────────────────────────────────────────────────────────
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

    // ── Built-in identifiers (contextually reserved) ──────────────────────────
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

    // ── Async / generator keywords (contextual) ───────────────────────────────
    Async,
    Await,
    Sync,
    Yield,

    // ── Dart 3.x pattern keyword ──────────────────────────────────────────────
    When,

    // ── Additional contextual keywords ───────────────────────────────────────
    On,       // `on` in mixin on-clause and try/catch
    Override, // `override` built-in identifier

    // ── Arithmetic ────────────────────────────────────────────────────────────
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    TildeSlash, // ~/

    // ── Comparison ────────────────────────────────────────────────────────────
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,

    // ── Logical ───────────────────────────────────────────────────────────────
    AmpAmp,
    PipePipe,
    Bang,

    // ── Bitwise ───────────────────────────────────────────────────────────────
    Amp,
    Pipe,
    Caret,
    Tilde,
    LtLt,
    GtGt,
    GtGtGt,

    // ── Assignment ────────────────────────────────────────────────────────────
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

    // ── Null / conditional ────────────────────────────────────────────────────
    Qmark,         // ?
    QmarkQmark,    // ??
    QmarkDot,      // ?.
    QmarkLBracket, // ?[

    // ── Increment / decrement ─────────────────────────────────────────────────
    PlusPlus,
    MinusMinus,

    // ── Punctuation ───────────────────────────────────────────────────────────
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

    // ── Dot family ────────────────────────────────────────────────────────────
    Dot,
    DotDot,         // ..  cascade
    DotDotQmark,    // ?.. null-safe cascade
    DotDotDot,      // ... spread
    DotDotDotQmark, // ...? null-aware spread

    // ── Arrow ─────────────────────────────────────────────────────────────────
    Arrow, // =>

    // ── Trivia ────────────────────────────────────────────────────────────────
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    DocComment,

    // ── Special ───────────────────────────────────────────────────────────────
    Eof,
    Error,
}

impl TokenKind {
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

    /// True if this token kind is a keyword or built-in identifier.
    pub fn is_keyword(&self) -> bool {
        !matches!(
            self,
            TokenKind::Ident
                | TokenKind::IntLit
                | TokenKind::DoubleLit
                | TokenKind::StringLit
                | TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::DocComment
                | TokenKind::Eof
                | TokenKind::Error
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::TildeSlash
                | TokenKind::EqEq
                | TokenKind::BangEq
                | TokenKind::Lt
                | TokenKind::Gt
                | TokenKind::LtEq
                | TokenKind::GtEq
                | TokenKind::AmpAmp
                | TokenKind::PipePipe
                | TokenKind::Bang
                | TokenKind::Amp
                | TokenKind::Pipe
                | TokenKind::Caret
                | TokenKind::Tilde
                | TokenKind::LtLt
                | TokenKind::GtGt
                | TokenKind::GtGtGt
                | TokenKind::Eq
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq
                | TokenKind::PercentEq
                | TokenKind::TildeSlashEq
                | TokenKind::AmpEq
                | TokenKind::PipeEq
                | TokenKind::CaretEq
                | TokenKind::LtLtEq
                | TokenKind::GtGtEq
                | TokenKind::GtGtGtEq
                | TokenKind::QmarkQmarkEq
                | TokenKind::Qmark
                | TokenKind::QmarkQmark
                | TokenKind::QmarkDot
                | TokenKind::QmarkLBracket
                | TokenKind::PlusPlus
                | TokenKind::MinusMinus
                | TokenKind::LParen
                | TokenKind::RParen
                | TokenKind::LBrace
                | TokenKind::RBrace
                | TokenKind::LBracket
                | TokenKind::RBracket
                | TokenKind::Comma
                | TokenKind::Semicolon
                | TokenKind::Colon
                | TokenKind::At
                | TokenKind::Hash
                | TokenKind::Dot
                | TokenKind::DotDot
                | TokenKind::DotDotQmark
                | TokenKind::DotDotDot
                | TokenKind::DotDotDotQmark
                | TokenKind::Arrow
                | TokenKind::On
                | TokenKind::Override
        )
    }
}
