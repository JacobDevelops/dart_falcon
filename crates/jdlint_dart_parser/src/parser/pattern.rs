use jdlint_syntax::ast::*;
use jdlint_syntax::token::TokenKind;

use super::Parser;

impl<'src> Parser<'src> {
    /// Entry point for pattern parsing (Dart 3.x).
    ///
    /// Grammar (simplified):
    ///   pattern  ::= logicalOrPattern
    ///   logicalOrPattern  ::= logicalAndPattern ('|' logicalAndPattern)*
    ///   logicalAndPattern ::= relationalPattern ('&' relationalPattern)*
    pub(super) fn parse_pattern(&mut self) -> Pattern {
        self.parse_logical_or_pattern()
    }

    fn parse_logical_or_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        let mut left = self.parse_logical_and_pattern();
        while self.eat(TokenKind::Pipe).is_some() {
            let right = self.parse_logical_and_pattern();
            let span = self.span_from(start);
            left = Pattern::LogicalOr { left: Box::new(left), right: Box::new(right), span };
        }
        left
    }

    fn parse_logical_and_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        let mut left = self.parse_relational_pattern();
        while self.eat(TokenKind::Amp).is_some() {
            let right = self.parse_relational_pattern();
            let span = self.span_from(start);
            left = Pattern::LogicalAnd { left: Box::new(left), right: Box::new(right), span };
        }
        left
    }

    fn parse_relational_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        // Relational: == expr, != expr, < expr, > expr, <= expr, >= expr
        let op = match self.cur().kind {
            TokenKind::EqEq => Some(RelationalPatternOp::Eq),
            TokenKind::BangEq => Some(RelationalPatternOp::NotEq),
            TokenKind::Lt => Some(RelationalPatternOp::Lt),
            TokenKind::Gt => Some(RelationalPatternOp::Gt),
            TokenKind::LtEq => Some(RelationalPatternOp::LtEq),
            TokenKind::GtEq => Some(RelationalPatternOp::GtEq),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let value = self.parse_expr();
            return Pattern::Relational { op, value, span: self.span_from(start) };
        }
        self.parse_postfix_pattern()
    }

    fn parse_postfix_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        let mut inner = self.parse_primary_pattern();
        loop {
            if self.eat(TokenKind::Qmark).is_some() {
                let span = self.span_from(start);
                inner = Pattern::NullCheck { inner: Box::new(inner), span };
            } else if self.eat(TokenKind::Bang).is_some() {
                let span = self.span_from(start);
                inner = Pattern::NullAssert { inner: Box::new(inner), span };
            } else if self.eat(TokenKind::As).is_some() {
                let cast_type = self.parse_type();
                let span = self.span_from(start);
                inner = Pattern::Cast { inner: Box::new(inner), cast_type, span };
            } else {
                break;
            }
        }
        inner
    }

    fn parse_primary_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;

        // Parenthesised / record pattern
        if self.at(TokenKind::LParen) {
            return self.parse_paren_or_record_pattern(start);
        }

        // List pattern: [elements]
        if self.at(TokenKind::LBracket) {
            return self.parse_list_pattern(start);
        }

        // Map pattern: {key: pattern, ...}
        if self.at(TokenKind::LBrace) {
            return self.parse_map_pattern(start);
        }

        // Null literal
        if self.eat(TokenKind::Null).is_some() {
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::Null,
                span: self.span_from(start),
            });
        }

        // Bool literals
        if self.eat(TokenKind::True).is_some() {
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::Bool(true),
                span: self.span_from(start),
            });
        }
        if self.eat(TokenKind::False).is_some() {
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::Bool(false),
                span: self.span_from(start),
            });
        }

        // Negative number literal: -intLit or -doubleLit
        if self.at(TokenKind::Minus) && matches!(self.peek(1).kind, TokenKind::IntLit | TokenKind::DoubleLit) {
            self.advance(); // -
            let tok = self.advance();
            let text = self.tok_text(&tok).to_string();
            let value = if tok.kind == TokenKind::IntLit {
                LiteralPatternValue::NegInt(text)
            } else {
                LiteralPatternValue::NegDouble(text)
            };
            return Pattern::Literal(LiteralPattern { value, span: self.span_from(start) });
        }

        // Number literals
        if self.at(TokenKind::IntLit) {
            let tok = self.advance();
            let text = self.tok_text(&tok).to_string();
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::Int(text),
                span: self.span_from(start),
            });
        }
        if self.at(TokenKind::DoubleLit) {
            let tok = self.advance();
            let text = self.tok_text(&tok).to_string();
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::Double(text),
                span: self.span_from(start),
            });
        }

        // String literal
        if self.at(TokenKind::StringLit) {
            let node = self.parse_string_lit();
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::String(node),
                span: self.span_from(start),
            });
        }

        // const keyword → const constructor or const value pattern
        if self.at(TokenKind::Const) {
            return self.parse_const_pattern(start);
        }

        // Wildcard: _
        if self.at(TokenKind::Ident) && self.cur_text() == "_" {
            self.advance();
            return Pattern::Wildcard { type_: None, span: self.span_from(start) };
        }

        // var / final → variable pattern  (var x, final x, final Type x)
        if self.at(TokenKind::Var) {
            self.advance();
            // var _ is a wildcard
            if self.at(TokenKind::Ident) && self.cur_text() == "_" {
                self.advance();
                return Pattern::Wildcard { type_: None, span: self.span_from(start) };
            }
            let name = self.expect_ident();
            return Pattern::Variable { type_: None, name, span: self.span_from(start) };
        }
        if self.at(TokenKind::Final) {
            self.advance();
            let var_type = if self.is_type_start() {
                let saved = self.pos;
                let ty = self.parse_type();
                if self.is_ident_like() { Some(ty) } else { self.pos = saved; None }
            } else {
                None
            };
            if self.at(TokenKind::Ident) && self.cur_text() == "_" {
                self.advance();
                return Pattern::Wildcard { type_: var_type, span: self.span_from(start) };
            }
            let name = self.expect_ident();
            return Pattern::Variable { type_: var_type, name, span: self.span_from(start) };
        }

        // Named type → either object pattern `Type(...)` or typed variable/wildcard `Type _` / `Type name`
        if self.is_type_start() {
            return self.parse_type_led_pattern(start);
        }

        self.error(format!("expected pattern, got {:?}", self.cur().kind));
        Pattern::Error { span: self.span_from(start) }
    }

    fn parse_paren_or_record_pattern(&mut self, start: usize) -> Pattern {
        self.advance(); // (
        if self.at(TokenKind::RParen) {
            self.advance();
            // Empty record pattern ()
            return Pattern::Record(RecordPattern { fields: Vec::new(), span: self.span_from(start) });
        }

        // Peek: if it looks like a record (has named fields or multiple positional), parse as record
        // Otherwise it's a parenthesised pattern
        let saved = self.pos;

        // Try named field first: `name: pattern`
        if (self.is_ident_like() || self.at(TokenKind::Ident)) && self.peek(1).kind == TokenKind::Colon {
            return self.parse_record_pattern_body(start);
        }

        // Parse first element
        let first = self.parse_pattern();

        if self.at(TokenKind::Comma) {
            // More elements → record pattern
            let _ = saved;
            let mut fields = vec![RecordPatternField { name: None, pattern: first, span: self.span_from(start) }];
            while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
                let fs = self.cur().offset;
                let name = if (self.is_ident_like() || self.at(TokenKind::Ident)) && self.peek(1).kind == TokenKind::Colon {
                    let n = self.expect_ident();
                    self.advance(); // :
                    Some(n)
                } else {
                    None
                };
                let pattern = self.parse_pattern();
                fields.push(RecordPatternField { name, pattern, span: self.span_from(fs) });
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RParen);
            Pattern::Record(RecordPattern { fields, span: self.span_from(start) })
        } else {
            // Single element → parenthesised pattern
            self.expect(TokenKind::RParen);
            Pattern::ParenPattern { inner: Box::new(first), span: self.span_from(start) }
        }
    }

    fn parse_record_pattern_body(&mut self, start: usize) -> Pattern {
        let mut fields = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            let fs = self.cur().offset;
            let name = if (self.is_ident_like() || self.at(TokenKind::Ident)) && self.peek(1).kind == TokenKind::Colon {
                let n = self.expect_ident();
                self.advance(); // :
                Some(n)
            } else {
                None
            };
            let pattern = self.parse_pattern();
            fields.push(RecordPatternField { name, pattern, span: self.span_from(fs) });
            if self.eat(TokenKind::Comma).is_none() { break; }
        }
        self.expect(TokenKind::RParen);
        Pattern::Record(RecordPattern { fields, span: self.span_from(start) })
    }

    fn parse_list_pattern(&mut self, start: usize) -> Pattern {
        self.advance(); // [
        let type_arg = None; // Type args come via object pattern; bare list patterns have no explicit type arg

        let mut elements = Vec::new();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            let es = self.cur().offset;
            if self.at(TokenKind::DotDotDot) {
                self.advance();
                let rest_pat = if !self.at(TokenKind::RBracket) && !self.at(TokenKind::Comma) {
                    Some(self.parse_pattern())
                } else {
                    None
                };
                elements.push(ListPatternElement::Rest(rest_pat, self.span_from(es)));
            } else {
                elements.push(ListPatternElement::Pattern(self.parse_pattern()));
            }
            if self.eat(TokenKind::Comma).is_none() { break; }
        }
        self.expect(TokenKind::RBracket);
        Pattern::List(ListPattern { type_arg, elements, span: self.span_from(start) })
    }

    fn parse_map_pattern(&mut self, start: usize) -> Pattern {
        self.advance(); // {
        let type_args = Vec::new();
        let mut entries = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            // Rest entry: ...
            if self.at(TokenKind::DotDotDot) {
                self.advance();
                if self.eat(TokenKind::Comma).is_none() { break; }
                continue;
            }
            let es = self.cur().offset;
            let key = self.parse_expr();
            self.expect(TokenKind::Colon);
            let pattern = self.parse_pattern();
            entries.push(MapPatternEntry { key, pattern, span: self.span_from(es) });
            if self.eat(TokenKind::Comma).is_none() { break; }
        }
        self.expect(TokenKind::RBrace);
        Pattern::Map(MapPattern { type_args, entries, span: self.span_from(start) })
    }

    fn parse_const_pattern(&mut self, start: usize) -> Pattern {
        self.advance(); // const
        // const ident.ident... or const constructor(...)
        let mut name = Vec::new();
        if self.is_ident_like() {
            name.push(self.expect_ident());
            while self.at(TokenKind::Dot) && self.is_ident_like_at(1) {
                self.advance(); // .
                name.push(self.expect_ident());
            }
        }
        Pattern::Const(ConstPattern { name, span: self.span_from(start) })
    }

    fn parse_type_led_pattern(&mut self, start: usize) -> Pattern {
        // Could be:
        //   TypeName(fields)       → object pattern
        //   TypeName _             → typed wildcard
        //   TypeName varName       → typed variable
        //   TypeName<T>[...]       → typed list pattern (rare)
        let ty = self.parse_type();

        if self.at(TokenKind::LParen) {
            // Object pattern: Type(field: pattern, ...)
            self.advance(); // (
            let mut fields = Vec::new();
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                let fs = self.cur().offset;
                let name = if (self.is_ident_like() || self.at(TokenKind::Ident)) && self.peek(1).kind == TokenKind::Colon {
                    let n = self.expect_ident();
                    self.advance(); // :
                    Some(n)
                } else {
                    None
                };
                let pattern = if self.at(TokenKind::RParen) || self.at(TokenKind::Comma) {
                    // shorthand: just the field name, no pattern
                    None
                } else {
                    Some(self.parse_pattern())
                };
                fields.push(ObjectPatternField {
                    name: name.unwrap_or_else(|| Identifier::new("<field>", self.cur_span())),
                    pattern,
                    span: self.span_from(fs),
                });
                if self.eat(TokenKind::Comma).is_none() { break; }
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RParen);
            return Pattern::Object(ObjectPattern { type_: ty, fields, span: self.span_from(start) });
        }

        // Wildcard typed: Type _
        if self.at(TokenKind::Ident) && self.cur_text() == "_" {
            self.advance();
            return Pattern::Wildcard { type_: Some(ty), span: self.span_from(start) };
        }

        // Typed variable: Type name
        if self.is_ident_like() {
            let name = self.expect_ident();
            return Pattern::Variable { type_: Some(ty), name, span: self.span_from(start) };
        }

        // Just a type by itself — treat as const pattern with type name
        match ty {
            DartType::Named(ref nt) => {
                let name = nt.segments.clone();
                Pattern::Const(ConstPattern { name, span: self.span_from(start) })
            }
            _ => Pattern::Error { span: self.span_from(start) },
        }
    }
}
