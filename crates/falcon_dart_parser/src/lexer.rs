pub(crate) mod scan;

use falcon_syntax::token::{Token, TokenKind};

/// Dart lexer preserving trivia and emitting `Error` tokens for malformed input.
pub struct Lexer<'src> {
    src: &'src str,
    pos: usize,
    /// A shebang may follow a leading BOM.
    logical_start: usize,
}

impl<'src> Lexer<'src> {
    /// # Panics
    ///
    /// Panics if the source exceeds the compact token limit of `u32::MAX` bytes.
    pub fn new(src: &'src str) -> Self {
        u32::try_from(src.len()).expect("source exceeds u32 token capacity");
        // Only a leading BOM is valid source.
        let pos = if src.starts_with('\u{FEFF}') {
            '\u{FEFF}'.len_utf8()
        } else {
            0
        };
        Self {
            src,
            pos,
            logical_start: pos,
        }
    }

    /// Lex the full source and return all tokens, ending with a single `Eof`.
    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token();
            let done = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if done {
                break;
            }
        }
        tokens
    }

    fn remaining(&self) -> &str {
        &self.src[self.pos..]
    }

    fn cur(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.remaining().chars().nth(offset)
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.cur()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat_if(&mut self, c: char) -> bool {
        if self.cur() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn make(&self, kind: TokenKind, start: usize) -> Token {
        Token::new(kind, start, self.pos - start)
    }

    /// Read one token, returning `Eof` after the source is exhausted.
    pub fn next_token(&mut self) -> Token {
        let start = self.pos;

        // A shebang is trivia only at the logical start, including after a BOM.
        if start == self.logical_start && self.remaining().starts_with("#!") {
            self.skip_line();
            return self.make(TokenKind::LineComment, start);
        }

        let c = match self.cur() {
            None => return self.make(TokenKind::Eof, start),
            Some(c) => c,
        };

        match c {
            ' ' | '\t' => {
                while matches!(self.cur(), Some(' ' | '\t')) {
                    self.advance();
                }
                self.make(TokenKind::Whitespace, start)
            }
            '\n' | '\r' => {
                self.advance();
                if c == '\r' && self.cur() == Some('\n') {
                    self.advance();
                }
                self.make(TokenKind::Newline, start)
            }
            '/' => self.lex_slash(start),
            '"' | '\'' => self.lex_string(c, false, start),
            'r' if matches!(self.peek(1), Some('"' | '\'')) => {
                self.advance();
                self.lex_string(self.cur().unwrap(), true, start)
            }
            '0'..='9' => self.lex_number(start),
            c if is_ident_start(c) => self.lex_ident(start),
            _ => self.lex_punct(start),
        }
    }

    fn lex_slash(&mut self, start: usize) -> Token {
        self.advance();
        match self.cur() {
            Some('/') if self.peek(1) == Some('/') => {
                self.advance();
                self.advance();
                self.skip_line();
                self.make(TokenKind::DocComment, start)
            }
            Some('/') => {
                self.advance();
                self.skip_line();
                self.make(TokenKind::LineComment, start)
            }
            // `/**/` is an empty block comment, not a documentation comment.
            Some('*') => {
                self.advance();
                let is_doc = self.cur() == Some('*') && self.peek(1) != Some('/');
                if !self.skip_block_comment(start) {
                    self.make(TokenKind::Error, start)
                } else if is_doc {
                    self.make(TokenKind::DocComment, start)
                } else {
                    self.make(TokenKind::BlockComment, start)
                }
            }
            Some('=') => {
                self.advance();
                self.make(TokenKind::SlashEq, start)
            }
            _ => self.make(TokenKind::Slash, start),
        }
    }

    fn lex_string(&mut self, quote: char, raw: bool, start: usize) -> Token {
        self.advance();
        let triple = self.cur() == Some(quote) && self.peek(1) == Some(quote);
        if triple {
            self.advance();
            self.advance();
        }
        loop {
            match self.cur() {
                None => return self.make(TokenKind::Error, start),
                Some('\n' | '\r') if !triple => return self.make(TokenKind::Error, start),
                Some('\\') if !raw => {
                    self.advance();
                    self.advance();
                }
                Some('$') if !raw => {
                    if !self.lex_interpolation() {
                        return self.make(TokenKind::Error, start);
                    }
                }
                Some(c) if c == quote => {
                    if !triple || (self.peek(1) == Some(quote) && self.peek(2) == Some(quote)) {
                        self.advance();
                        if triple {
                            self.advance();
                            self.advance();
                        }
                        return self.make(TokenKind::StringLit, start);
                    }
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn lex_interpolation(&mut self) -> bool {
        self.advance();
        match self.cur() {
            Some('{') => {
                let close = scan::interp_close(self.src.as_bytes(), self.pos, self.src.len());
                self.pos = close.map_or(self.src.len(), |close| close + 1);
                close.is_some()
            }
            Some(c) if is_ident_start(c) => {
                // `$` starts the next interpolation rather than continuing this name.
                while matches!(self.cur(), Some(c) if is_ident_continue(c) && c != '$') {
                    self.advance();
                }
                true
            }
            _ => true,
        }
    }

    fn skip_line(&mut self) {
        self.pos = scan::skip_line(self.src.as_bytes(), self.pos, self.src.len());
    }

    fn skip_block_comment(&mut self, open: usize) -> bool {
        let end = scan::skip_block_comment(self.src.as_bytes(), open, self.src.len());
        self.pos = end.unwrap_or(self.src.len());
        end.is_some()
    }

    fn lex_number(&mut self, start: usize) -> Token {
        if self.cur() == Some('0') && matches!(self.peek(1), Some('x' | 'X')) {
            self.advance();
            self.advance();
            while matches!(self.cur(), Some('0'..='9' | 'a'..='f' | 'A'..='F' | '_')) {
                self.advance();
            }
            return self.make(TokenKind::IntLit, start);
        }

        self.skip_digits();
        // A fractional dot must precede a digit, not a cascade.
        let fractional = self.cur() == Some('.') && matches!(self.peek(1), Some('0'..='9'));
        if fractional {
            self.advance();
            self.skip_digits();
        }
        let exponent = self.lex_exponent();
        self.make(
            if fractional || exponent {
                TokenKind::DoubleLit
            } else {
                TokenKind::IntLit
            },
            start,
        )
    }

    fn skip_digits(&mut self) {
        while matches!(self.cur(), Some('0'..='9' | '_')) {
            self.advance();
        }
    }

    fn lex_exponent(&mut self) -> bool {
        if !matches!(self.cur(), Some('e' | 'E')) {
            return false;
        }
        self.advance();
        if matches!(self.cur(), Some('+' | '-')) {
            self.advance();
        }
        self.skip_digits();
        true
    }

    fn lex_ident(&mut self, start: usize) -> Token {
        while matches!(self.cur(), Some(c) if is_ident_continue(c)) {
            self.advance();
        }
        let text = &self.src[start..self.pos];
        let kind = TokenKind::from_keyword(text).unwrap_or(TokenKind::Ident);
        self.make(kind, start)
    }

    fn lex_punct(&mut self, start: usize) -> Token {
        let c = self.cur().unwrap();
        self.advance();

        let kind = match c {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '@' => TokenKind::At,
            ':' => TokenKind::Colon,
            '#' => TokenKind::Hash,

            '+' => {
                if self.eat_if('+') {
                    TokenKind::PlusPlus
                } else if self.eat_if('=') {
                    TokenKind::PlusEq
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if self.eat_if('-') {
                    TokenKind::MinusMinus
                } else if self.eat_if('=') {
                    TokenKind::MinusEq
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if self.eat_if('=') {
                    TokenKind::StarEq
                } else {
                    TokenKind::Star
                }
            }
            '%' => {
                if self.eat_if('=') {
                    TokenKind::PercentEq
                } else {
                    TokenKind::Percent
                }
            }
            '~' => {
                if self.eat_if('/') {
                    if self.eat_if('=') {
                        TokenKind::TildeSlashEq
                    } else {
                        TokenKind::TildeSlash
                    }
                } else {
                    TokenKind::Tilde
                }
            }
            '=' => {
                if self.eat_if('=') {
                    TokenKind::EqEq
                } else if self.eat_if('>') {
                    TokenKind::Arrow
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if self.eat_if('=') {
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.eat_if('<') {
                    if self.eat_if('=') {
                        TokenKind::LtLtEq
                    } else {
                        TokenKind::LtLt
                    }
                } else if self.eat_if('=') {
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.cur() == Some('>') && self.peek(1) == Some('>') {
                    self.advance();
                    self.advance();
                    if self.eat_if('=') {
                        TokenKind::GtGtGtEq
                    } else {
                        TokenKind::GtGtGt
                    }
                } else if self.eat_if('>') {
                    if self.eat_if('=') {
                        TokenKind::GtGtEq
                    } else {
                        TokenKind::GtGt
                    }
                } else if self.eat_if('=') {
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                if self.eat_if('&') {
                    TokenKind::AmpAmp
                } else if self.eat_if('=') {
                    TokenKind::AmpEq
                } else {
                    TokenKind::Amp
                }
            }
            '|' => {
                if self.eat_if('|') {
                    TokenKind::PipePipe
                } else if self.eat_if('=') {
                    TokenKind::PipeEq
                } else {
                    TokenKind::Pipe
                }
            }
            '^' => {
                if self.eat_if('=') {
                    TokenKind::CaretEq
                } else {
                    TokenKind::Caret
                }
            }
            '?' => {
                if self.eat_if('?') {
                    if self.eat_if('=') {
                        TokenKind::QmarkQmarkEq
                    } else {
                        TokenKind::QmarkQmark
                    }
                } else if self.cur() == Some('.') && self.peek(1) == Some('.') {
                    self.advance();
                    self.advance();
                    TokenKind::DotDotQmark
                } else if self.eat_if('.') {
                    TokenKind::QmarkDot
                } else if self.eat_if('[') {
                    TokenKind::QmarkLBracket
                } else {
                    TokenKind::Qmark
                }
            }
            '.' => {
                if self.eat_if('.') {
                    if self.eat_if('.') {
                        if self.eat_if('?') {
                            TokenKind::DotDotDotQmark
                        } else {
                            TokenKind::DotDotDot
                        }
                    } else {
                        TokenKind::DotDot
                    }
                } else if matches!(self.cur(), Some('0'..='9')) {
                    self.skip_digits();
                    self.lex_exponent();
                    return self.make(TokenKind::DoubleLit, start);
                } else {
                    TokenKind::Dot
                }
            }
            _ => TokenKind::Error,
        };

        self.make(kind, start)
    }
}

#[inline]
fn is_ident_start(c: char) -> bool {
    c == '_' || c == '$' || c.is_ascii_alphabetic()
}

#[inline]
fn is_ident_continue(c: char) -> bool {
    c == '_' || c == '$' || c.is_ascii_alphanumeric()
}
