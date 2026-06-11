use falcon_syntax::token::{Token, TokenKind};

/// Hand-rolled Dart 3.x lexer.
///
/// Produces a complete token stream including trivia (whitespace, comments).
/// Call `Lexer::new(source).tokenize()` to lex an entire file.  No panics on
/// malformed input — ill-formed tokens become `TokenKind::Error`.
pub struct Lexer<'src> {
    src: &'src str,
    pos: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self { src, pos: 0 }
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

    // ── Cursor helpers ────────────────────────────────────────────────────────

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

    // ── Main dispatch ─────────────────────────────────────────────────────────

    fn next_token(&mut self) -> Token {
        let start = self.pos;

        let c = match self.cur() {
            None => return self.make(TokenKind::Eof, start),
            Some(c) => c,
        };

        match c {
            ' ' | '\t' | '\r' => {
                while matches!(self.cur(), Some(' ' | '\t' | '\r')) {
                    self.advance();
                }
                self.make(TokenKind::Whitespace, start)
            }
            '\n' => {
                self.advance();
                self.make(TokenKind::Newline, start)
            }
            '/' => self.lex_slash(start),
            '"' | '\'' => self.lex_string(c, false, start),
            'r' if matches!(self.peek(1), Some('"' | '\'')) => {
                self.advance(); // consume 'r'
                let q = self.cur().unwrap();
                self.advance(); // consume the quote
                let triple = self.cur() == Some(q) && self.peek(1) == Some(q);
                if triple {
                    self.advance();
                    self.advance();
                    self.lex_string_body_triple(q, true, start)
                } else {
                    self.lex_string_body_single(q, true, start)
                }
            }
            '0'..='9' => self.lex_number(start),
            c if is_ident_start(c) => self.lex_ident(start),
            _ => self.lex_punct(start),
        }
    }

    // ── Slash: /  //  ///  /* */  ~/  ────────────────────────────────────────

    fn lex_slash(&mut self, start: usize) -> Token {
        self.advance(); // consume '/'
        match self.cur() {
            // Doc comment: /// (peek two chars ahead before treating as //)
            Some('/') if self.peek(1) == Some('/') => {
                self.advance(); // second /
                self.advance(); // third /
                while !matches!(self.cur(), None | Some('\n')) {
                    self.advance();
                }
                self.make(TokenKind::DocComment, start)
            }
            // Line comment: //
            Some('/') => {
                self.advance(); // second /
                while !matches!(self.cur(), None | Some('\n')) {
                    self.advance();
                }
                self.make(TokenKind::LineComment, start)
            }
            // Block comment: /* ... */ (nested depth tracked)
            Some('*') => {
                self.advance(); // consume '*'
                let mut depth: usize = 1;
                loop {
                    match (self.cur(), self.peek(1)) {
                        (None, _) => break, // unterminated
                        (Some('/'), Some('*')) => {
                            self.advance();
                            self.advance();
                            depth += 1;
                        }
                        (Some('*'), Some('/')) => {
                            self.advance();
                            self.advance();
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {
                            self.advance();
                        }
                    }
                }
                if depth == 0 {
                    self.make(TokenKind::BlockComment, start)
                } else {
                    self.make(TokenKind::Error, start)
                }
            }
            // /=
            Some('=') => {
                self.advance();
                self.make(TokenKind::SlashEq, start)
            }
            // plain /
            _ => self.make(TokenKind::Slash, start),
        }
    }

    // ── Strings ───────────────────────────────────────────────────────────────

    fn lex_string(&mut self, quote: char, raw: bool, start: usize) -> Token {
        self.advance(); // consume opening quote
        let triple = self.cur() == Some(quote) && self.peek(1) == Some(quote);
        if triple {
            self.advance(); // second quote
            self.advance(); // third quote
            self.lex_string_body_triple(quote, raw, start)
        } else {
            self.lex_string_body_single(quote, raw, start)
        }
    }

    fn lex_string_body_single(&mut self, quote: char, raw: bool, start: usize) -> Token {
        loop {
            match self.cur() {
                None | Some('\n') => {
                    return self.make(TokenKind::Error, start);
                }
                Some('\\') if !raw => {
                    self.advance(); // backslash
                    self.advance(); // escaped char
                }
                Some('$') if !raw => {
                    self.advance(); // '$'
                    match self.cur() {
                        Some('{') => {
                            self.advance(); // '{'
                            let mut depth: usize = 1;
                            while depth > 0 {
                                match self.cur() {
                                    None => return self.make(TokenKind::Error, start),
                                    Some('{') => {
                                        self.advance();
                                        depth += 1;
                                    }
                                    Some('}') => {
                                        self.advance();
                                        depth -= 1;
                                    }
                                    Some('\\') => {
                                        self.advance();
                                        self.advance();
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            }
                        }
                        Some(c) if is_ident_start(c) => {
                            while matches!(self.cur(), Some(c) if is_ident_continue(c)) {
                                self.advance();
                            }
                        }
                        _ => {}
                    }
                }
                Some(c) if c == quote => {
                    self.advance(); // closing quote
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
        self.make(TokenKind::StringLit, start)
    }

    fn lex_string_body_triple(&mut self, quote: char, _raw: bool, start: usize) -> Token {
        loop {
            match self.cur() {
                None => return self.make(TokenKind::Error, start),
                Some(c) if c == quote => {
                    if self.peek(1) == Some(quote) && self.peek(2) == Some(quote) {
                        self.advance();
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        self.make(TokenKind::StringLit, start)
    }

    // ── Numbers ───────────────────────────────────────────────────────────────

    fn lex_number(&mut self, start: usize) -> Token {
        // Hex literal: 0x…
        if self.cur() == Some('0') && matches!(self.peek(1), Some('x' | 'X')) {
            self.advance(); // 0
            self.advance(); // x / X
            let hex_start = self.pos;
            while matches!(self.cur(), Some('0'..='9' | 'a'..='f' | 'A'..='F' | '_')) {
                self.advance();
            }
            let _ = hex_start;
            return self.make(TokenKind::IntLit, start);
        }

        // Integer part
        while matches!(self.cur(), Some('0'..='9' | '_')) {
            self.advance();
        }

        // Fractional part: only if digit follows the dot (not a cascade ..)
        if self.cur() == Some('.') && matches!(self.peek(1), Some('0'..='9')) {
            self.advance(); // dot
            while matches!(self.cur(), Some('0'..='9' | '_')) {
                self.advance();
            }
            // Fall through to exponent check → will return DoubleLit
        }

        // Exponent
        if matches!(self.cur(), Some('e' | 'E')) {
            self.advance();
            if matches!(self.cur(), Some('+' | '-')) {
                self.advance();
            }
            while matches!(self.cur(), Some('0'..='9' | '_')) {
                self.advance();
            }
            return self.make(TokenKind::DoubleLit, start);
        }

        // If we consumed a dot (fractional) the text now has a '.' in it → double
        let text = &self.src[start..self.pos];
        if text.contains('.') {
            self.make(TokenKind::DoubleLit, start)
        } else {
            self.make(TokenKind::IntLit, start)
        }
    }

    // ── Identifiers / keywords ────────────────────────────────────────────────

    fn lex_ident(&mut self, start: usize) -> Token {
        while matches!(self.cur(), Some(c) if is_ident_continue(c)) {
            self.advance();
        }
        let text = &self.src[start..self.pos];
        let kind = TokenKind::from_keyword(text).unwrap_or(TokenKind::Ident);
        self.make(kind, start)
    }

    // ── Operators and punctuation ─────────────────────────────────────────────

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
                // >>>  >>  >>=  >>>=  >=  >
                if self.cur() == Some('>') && self.peek(1) == Some('>') {
                    self.advance(); // second >
                    self.advance(); // third >
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
                    self.advance(); // .
                    self.advance(); // .
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
                    // .5 style double literal
                    while matches!(self.cur(), Some('0'..='9' | '_')) {
                        self.advance();
                    }
                    if matches!(self.cur(), Some('e' | 'E')) {
                        self.advance();
                        if matches!(self.cur(), Some('+' | '-')) {
                            self.advance();
                        }
                        while matches!(self.cur(), Some('0'..='9')) {
                            self.advance();
                        }
                    }
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

// ── Character class helpers ───────────────────────────────────────────────────

#[inline]
pub fn is_ident_start(c: char) -> bool {
    c == '_' || c == '$' || c.is_alphabetic()
}

#[inline]
pub fn is_ident_continue(c: char) -> bool {
    c == '_' || c == '$' || c.is_alphanumeric()
}

// ── Convenience: filter trivia ────────────────────────────────────────────────

/// Returns the token stream with whitespace, newline, and comment tokens
/// removed.  The final `Eof` token is preserved.
pub fn filter_trivia(tokens: Vec<Token>) -> Vec<Token> {
    tokens.into_iter().filter(|t| !t.is_trivia()).collect()
}
