/// A lexical token in Dart source.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub offset: usize,
    pub len: usize,
}

/// Token kinds for Dart 3.x.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Populated during M1 lexer implementation
}
