use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::Parser;

impl<'src> Parser<'src> {
    /// Entry point for pattern parsing (Dart 3.x).
    ///
    /// Grammar (simplified):
    ///   pattern  ::= logicalOrPattern
    ///   logicalOrPattern  ::= logicalAndPattern ('||' logicalAndPattern)*
    ///   logicalAndPattern ::= relationalPattern ('&&' relationalPattern)*
    pub(super) fn parse_pattern(&mut self) -> Pattern {
        self.parse_logical_or_pattern()
    }

    /// Parse the irrefutable pattern of a pattern-variable declaration or pattern
    /// for-in header. In this context a bare identifier is a binding
    /// ([`Pattern::Variable`]) rather than a constant reference.
    pub(super) fn parse_binding_pattern(&mut self) -> Pattern {
        let prev = self.pattern_binding;
        self.pattern_binding = true;
        let pat = self.parse_pattern();
        self.pattern_binding = prev;
        pat
    }

    fn parse_logical_or_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        let mut left = self.parse_logical_and_pattern();
        while self.eat(TokenKind::PipePipe).is_some() {
            let right = self.parse_logical_and_pattern();
            let span = self.span_from(start);
            left = Pattern::LogicalOr {
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        left
    }

    fn parse_logical_and_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        let mut left = self.parse_relational_pattern();
        while self.eat(TokenKind::AmpAmp).is_some() {
            let right = self.parse_relational_pattern();
            let span = self.span_from(start);
            left = Pattern::LogicalAnd {
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        left
    }

    fn parse_relational_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        // A `<` that opens a typed collection pattern (`<int>[...]`, `<K, V>{...}`)
        // is not the relational `<` operator — defer to the primary parser.
        if self.at(TokenKind::Lt) && self.typed_collection_ahead() {
            return self.parse_postfix_pattern();
        }
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
            // The operand is a `bitwiseOrExpression`: parsing at this tier keeps
            // `&&`/`||` at the pattern level so `> 3 && < 5` is a logical-and
            // pattern rather than the operand swallowing the `&&`.
            let value = self.parse_bitwise_or();
            return Pattern::Relational {
                op,
                value,
                span: self.span_from(start),
            };
        }
        self.parse_postfix_pattern()
    }

    /// True when the `<` at the cursor opens the type-argument list of a typed
    /// collection pattern — a balanced `<...>` immediately followed by `[` or
    /// `{`. Distinguishes `<int>[a, b]` from the relational `< expr` operator.
    fn typed_collection_ahead(&self) -> bool {
        use TokenKind::*;
        debug_assert_eq!(self.cur().kind, Lt);
        let mut depth = 0i32;
        let mut i = self.pos;
        while let Some(tok) = self.tokens.get(i) {
            let closed_at = match &tok.kind {
                Lt => {
                    depth += 1;
                    None
                }
                Gt => {
                    depth -= 1;
                    (depth <= 0).then_some(i + 1)
                }
                GtGt => {
                    depth -= 2;
                    (depth <= 0).then_some(i + 1)
                }
                GtGtGt => {
                    depth -= 3;
                    (depth <= 0).then_some(i + 1)
                }
                Eof => return false,
                _ => None,
            };
            if let Some(idx) = closed_at {
                return matches!(
                    self.tokens.get(idx).map(|t| &t.kind),
                    Some(LBracket | LBrace)
                );
            }
            i += 1;
            if i - self.pos > 512 {
                return false;
            }
        }
        false
    }

    fn parse_postfix_pattern(&mut self) -> Pattern {
        let start = self.cur().offset;
        let mut inner = self.parse_primary_pattern();
        loop {
            if self.eat(TokenKind::Qmark).is_some() {
                let span = self.span_from(start);
                inner = Pattern::NullCheck {
                    inner: Box::new(inner),
                    span,
                };
            } else if self.eat(TokenKind::Bang).is_some() {
                let span = self.span_from(start);
                inner = Pattern::NullAssert {
                    inner: Box::new(inner),
                    span,
                };
            } else if self.eat(TokenKind::As).is_some() {
                let cast_type = self.parse_type();
                let span = self.span_from(start);
                inner = Pattern::Cast {
                    inner: Box::new(inner),
                    cast_type,
                    span,
                };
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
            return self.parse_list_pattern(start, None);
        }

        // Map pattern: {key: pattern, ...}
        if self.at(TokenKind::LBrace) {
            return self.parse_map_pattern(start, Vec::new());
        }

        // Typed collection pattern: `<int>[a, b]` or `<String, int>{...}`.
        if self.at(TokenKind::Lt) {
            let type_args = self.parse_type_args();
            if self.at(TokenKind::LBracket) {
                return self.parse_list_pattern(start, type_args.into_iter().next());
            }
            if self.at(TokenKind::LBrace) {
                return self.parse_map_pattern(start, type_args);
            }
            self.error(format!(
                "expected '[' or '{{' after type arguments in collection pattern, got {:?}",
                self.cur().kind
            ));
            return Pattern::Error {
                span: self.span_from(start),
            };
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
        if self.at(TokenKind::Minus)
            && matches!(self.peek(1).kind, TokenKind::IntLit | TokenKind::DoubleLit)
        {
            self.advance(); // -
            let tok = self.advance();
            let text = self.tok_text(&tok).to_string();
            let value = if tok.kind == TokenKind::IntLit {
                LiteralPatternValue::NegInt(text)
            } else {
                LiteralPatternValue::NegDouble(text)
            };
            return Pattern::Literal(LiteralPattern {
                value,
                span: self.span_from(start),
            });
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
            return Pattern::Wildcard {
                type_: None,
                span: self.span_from(start),
            };
        }

        // var / final → variable pattern  (var x, final x, final Type x)
        if self.at(TokenKind::Var) {
            self.advance();
            // var _ is a wildcard
            if self.at(TokenKind::Ident) && self.cur_text() == "_" {
                self.advance();
                return Pattern::Wildcard {
                    type_: None,
                    span: self.span_from(start),
                };
            }
            let name = self.expect_ident();
            return Pattern::Variable {
                type_: None,
                name,
                span: self.span_from(start),
            };
        }
        if self.at(TokenKind::Final) {
            self.advance();
            let var_type = if self.is_type_start() {
                let saved = self.pos;
                let ty = self.parse_type();
                // `when` here introduces a case guard, not a variable name.
                if self.is_ident_like() && !self.at(TokenKind::When) {
                    Some(ty)
                } else {
                    self.pos = saved;
                    None
                }
            } else {
                None
            };
            if self.at(TokenKind::Ident) && self.cur_text() == "_" {
                self.advance();
                return Pattern::Wildcard {
                    type_: var_type,
                    span: self.span_from(start),
                };
            }
            let name = self.expect_ident();
            return Pattern::Variable {
                type_: var_type,
                name,
                span: self.span_from(start),
            };
        }

        // Named type → either object pattern `Type(...)` or typed variable/wildcard `Type _` / `Type name`
        if self.is_type_start() {
            return self.parse_type_led_pattern(start);
        }

        self.error(format!("expected pattern, got {:?}", self.cur().kind));
        Pattern::Error {
            span: self.span_from(start),
        }
    }

    fn parse_paren_or_record_pattern(&mut self, start: usize) -> Pattern {
        self.advance(); // (
        if self.at(TokenKind::RParen) {
            self.advance();
            // Empty record pattern ()
            return Pattern::Record(RecordPattern {
                fields: Vec::new(),
                span: self.span_from(start),
            });
        }

        // A leading `:name` shorthand or `name:` named field means this is a
        // record pattern (never a parenthesised single pattern).
        if self.at(TokenKind::Colon)
            || ((self.is_ident_like() || self.at(TokenKind::Ident))
                && self.peek(1).kind == TokenKind::Colon)
        {
            return self.parse_record_pattern_body(start);
        }

        // Parse first element
        let first = self.parse_pattern();

        if self.at(TokenKind::Comma) {
            // More elements → record pattern
            let mut fields = vec![RecordPatternField {
                name: None,
                pattern: first,
                span: self.span_from(start),
            }];
            while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
                fields.push(self.parse_record_field());
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RParen);
            Pattern::Record(RecordPattern {
                fields,
                span: self.span_from(start),
            })
        } else {
            // Single element → parenthesised pattern
            self.expect(TokenKind::RParen);
            Pattern::ParenPattern {
                inner: Box::new(first),
                span: self.span_from(start),
            }
        }
    }

    fn parse_record_pattern_body(&mut self, start: usize) -> Pattern {
        let mut fields = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            fields.push(self.parse_record_field());
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        Pattern::Record(RecordPattern {
            fields,
            span: self.span_from(start),
        })
    }

    /// Parse one record-pattern field: `:name` shorthand (a variable binding
    /// whose getter name is the bound name), `name: pattern`, or a positional
    /// `pattern`.
    fn parse_record_field(&mut self) -> RecordPatternField {
        let fs = self.cur().offset;
        if self.at(TokenKind::Colon) {
            self.advance(); // :
            // `:name` (also `:var name` / `:final name`) always binds a variable.
            let prev = self.pattern_binding;
            self.pattern_binding = true;
            let pattern = self.parse_pattern();
            self.pattern_binding = prev;
            let name = binding_pattern_name(&pattern);
            return RecordPatternField {
                name,
                pattern,
                span: self.span_from(fs),
            };
        }
        let name = if (self.is_ident_like() || self.at(TokenKind::Ident))
            && self.peek(1).kind == TokenKind::Colon
        {
            let n = self.expect_ident();
            self.advance(); // :
            Some(n)
        } else {
            None
        };
        let pattern = self.parse_pattern();
        RecordPatternField {
            name,
            pattern,
            span: self.span_from(fs),
        }
    }

    fn parse_list_pattern(&mut self, start: usize, type_arg: Option<DartType>) -> Pattern {
        self.advance(); // [

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
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RBracket);
        Pattern::List(ListPattern {
            type_arg,
            elements,
            span: self.span_from(start),
        })
    }

    fn parse_map_pattern(&mut self, start: usize, type_args: Vec<DartType>) -> Pattern {
        self.advance(); // {
        let mut has_rest = false;
        let mut entries = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            // Rest entry: ...
            if self.at(TokenKind::DotDotDot) {
                self.advance();
                has_rest = true;
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
                continue;
            }
            let es = self.cur().offset;
            let key = self.parse_expr();
            self.expect(TokenKind::Colon);
            let pattern = self.parse_pattern();
            entries.push(MapPatternEntry {
                key,
                pattern,
                span: self.span_from(es),
            });
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RBrace);
        Pattern::Map(MapPattern {
            type_args,
            entries,
            has_rest,
            span: self.span_from(start),
        })
    }

    fn parse_const_pattern(&mut self, start: usize) -> Pattern {
        // Parenthesized constant expression: `const (1 + 2)`. Handled here rather
        // than via `parse_primary`, which would read `(...)` as a record type.
        if self.peek(1).kind == TokenKind::LParen {
            self.advance(); // const
            self.advance(); // (
            let inner = self.parse_expr();
            self.expect(TokenKind::RParen);
            return Pattern::Const(ConstPattern {
                name: Vec::new(),
                expr: Some(Box::new(inner)),
                span: self.span_from(start),
            });
        }

        // Distinguish the expression forms (const constructor / collection
        // literal — `const Foo(1)`, `const [1, 2]`, `const {1}`, `const <int>[1]`)
        // from a bare dotted-name constant reference (`const Foo.bar`, the
        // existing `expr: None` form).
        let is_expr_form = match self.peek(1).kind {
            TokenKind::LBracket | TokenKind::LBrace | TokenKind::Lt => true,
            _ if self.is_ident_like_at(1) => {
                // A const constructor has `(` / `<` after the (dotted) name; a
                // bare constant reference does not.
                let mut i = 2;
                while self.peek(i).kind == TokenKind::Dot
                    && self.is_ident_like_kind(&self.peek(i + 1).kind)
                {
                    i += 2;
                }
                matches!(self.peek(i).kind, TokenKind::LParen | TokenKind::Lt)
            }
            _ => false,
        };

        if is_expr_form {
            // `parse_primary` consumes the `const` keyword itself and produces the
            // const constructor / collection-literal expression.
            let expr = self.parse_primary();
            return Pattern::Const(ConstPattern {
                name: Vec::new(),
                expr: Some(Box::new(expr)),
                span: self.span_from(start),
            });
        }

        // Bare dotted-name constant reference: `const Foo`, `const Foo.bar`.
        self.advance(); // const
        let mut name = Vec::new();
        if self.is_ident_like() {
            name.push(self.expect_ident());
            while self.at(TokenKind::Dot) && self.is_ident_like_at(1) {
                self.advance(); // .
                name.push(self.expect_ident());
            }
        }
        Pattern::Const(ConstPattern {
            name,
            expr: None,
            span: self.span_from(start),
        })
    }

    fn parse_type_led_pattern(&mut self, start: usize) -> Pattern {
        // Could be:
        //   TypeName(fields)       → object pattern
        //   TypeName _             → typed wildcard
        //   TypeName varName       → typed variable
        //   TypeName<T>[...]       → typed list pattern (rare)
        let ty = self.parse_type();

        if self.at(TokenKind::LParen) {
            // Object pattern: Type(field: pattern, ...) or Type(:var field, ...)
            self.advance(); // (
            let mut fields = Vec::new();
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                let fs = self.cur().offset;
                // Dart 3 colon-shorthand: `:field` binds a variable whose getter
                // name is the bound name (also `:var x` / `:final x`). Parse in
                // binding mode so `field` becomes a variable, not a constant
                // reference, in refutable contexts like `case Foo(:field)`.
                if self.at(TokenKind::Colon) {
                    self.advance(); // :
                    let prev = self.pattern_binding;
                    self.pattern_binding = true;
                    let inner = self.parse_pattern();
                    self.pattern_binding = prev;
                    let field_name = binding_pattern_name(&inner)
                        .unwrap_or_else(|| Identifier::new("<shorthand>", self.cur_span()));
                    fields.push(ObjectPatternField {
                        name: field_name,
                        pattern: Some(inner),
                        span: self.span_from(fs),
                    });
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                    continue;
                }
                let name = if (self.is_ident_like() || self.at(TokenKind::Ident))
                    && self.peek(1).kind == TokenKind::Colon
                {
                    let n = self.expect_ident();
                    self.advance(); // :
                    Some(n)
                } else {
                    None
                };
                let pattern = if self.at(TokenKind::RParen) || self.at(TokenKind::Comma) {
                    None
                } else {
                    Some(self.parse_pattern())
                };
                fields.push(ObjectPatternField {
                    name: name.unwrap_or_else(|| Identifier::new("<field>", self.cur_span())),
                    pattern,
                    span: self.span_from(fs),
                });
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.eat(TokenKind::Comma);
            self.expect(TokenKind::RParen);
            return Pattern::Object(ObjectPattern {
                type_: ty,
                fields,
                span: self.span_from(start),
            });
        }

        // Wildcard typed: Type _
        if self.at(TokenKind::Ident) && self.cur_text() == "_" {
            self.advance();
            return Pattern::Wildcard {
                type_: Some(ty),
                span: self.span_from(start),
            };
        }

        // Typed variable: Type name. `when` here introduces a case guard
        // (`case State.a when c`), not a variable name, so it must not be
        // consumed as the binding.
        if self.is_ident_like() && !self.at(TokenKind::When) {
            let name = self.expect_ident();
            return Pattern::Variable {
                type_: Some(ty),
                name,
                span: self.span_from(start),
            };
        }

        // Just a type by itself — treat as const pattern with type name. In a
        // binding context (`var (a, b) = ..`) a bare single identifier is a
        // variable binding, not a constant reference.
        match ty {
            DartType::Named(ref nt) => {
                if self.pattern_binding
                    && nt.segments.len() == 1
                    && nt.type_args.is_empty()
                    && !nt.is_nullable
                {
                    let name = nt.segments[0].clone();
                    Pattern::Variable {
                        type_: None,
                        name,
                        span: self.span_from(start),
                    }
                } else {
                    Pattern::Const(ConstPattern {
                        name: nt.segments.clone(),
                        expr: None,
                        span: self.span_from(start),
                    })
                }
            }
            _ => Pattern::Error {
                span: self.span_from(start),
            },
        }
    }
}

/// The bound name of a `:name`-shorthand record field, taken from the inner
/// variable/wildcard pattern.
fn binding_pattern_name(pattern: &Pattern) -> Option<Identifier> {
    match pattern {
        Pattern::Variable { name, .. } => Some(name.clone()),
        Pattern::Wildcard { .. } => None,
        _ => None,
    }
}
