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
    /// `u32::MAX` bytes, matching the syntax tree's source capacity.
    #[must_use]
    pub fn new(kind: TokenKind, offset: usize, len: usize) -> Self {
        Self {
            kind,
            offset: u32::try_from(offset).expect("token offset exceeds u32 source capacity"),
            len: u32::try_from(len).expect("token length exceeds u32 source capacity"),
        }
    }

    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset as usize
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.offset()..self.offset() + self.len()]
    }

    #[must_use]
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
    StringPart,
    InterpolationStart,
    InterpolationDollar,

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
    #[must_use]
    pub fn is_comment(self) -> bool {
        matches!(
            self,
            Self::LineComment | Self::BlockComment | Self::DocComment
        )
    }

    #[must_use]
    pub fn is_identifier_like(self) -> bool {
        use TokenKind::{
            Abstract, As, Async, Await, Base, Covariant, Deferred, Dynamic, Export, Extension,
            External, Factory, Function, Get, Hide, Ident, Implements, Import, Interface, Late,
            Library, Mixin, On, Operator, Override, Part, Required, Sealed, Set, Show, Static,
            Sync, Type, Typedef, When, Yield,
        };
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
    #[must_use]
    pub fn is_user_definable_operator(self) -> bool {
        use TokenKind::{
            Amp, Caret, EqEq, Gt, GtEq, GtGt, GtGtGt, Lt, LtEq, LtLt, Minus, Percent, Pipe, Plus,
            Slash, Star, Tilde, TildeSlash,
        };
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
    #[must_use]
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
mod tests {
    use super::*;

    #[test]
    fn lexical_token_layout_stays_compact() {
        assert_eq!(std::mem::size_of::<Token>(), 9);
        assert_eq!(std::mem::align_of::<Token>(), 1);
        assert_eq!(std::mem::size_of::<TokenKind>(), 1);
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

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "token offset exceeds u32 source capacity")]
    fn compact_token_rejects_offset_overflow() {
        let _ = Token::new(TokenKind::Eof, u32::MAX as usize + 1, 0);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "token length exceeds u32 source capacity")]
    fn compact_token_rejects_length_overflow() {
        let _ = Token::new(TokenKind::StringLit, 0, u32::MAX as usize + 1);
    }
}
