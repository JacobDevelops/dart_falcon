use falcon_syntax::ast::*;
use falcon_syntax::cst::NodeKind;
use falcon_syntax::token::TokenKind;

use super::{Parser, expr::Brackets};

impl<'src> Parser<'src> {
    /// Parses Dart 3 logical-or patterns.
    pub(super) fn parse_pattern(&mut self) -> Pattern {
        self.parse_logical_or_pattern()
    }

    // Declaration and for-in patterns bind bare names instead of referencing constants.
    pub(super) fn parse_binding_pattern(&mut self) -> Pattern {
        self.with_ctx(|ctx| ctx.pattern_binding = true, Self::parse_pattern)
    }

    fn parse_logical_or_pattern(&mut self) -> Pattern {
        let start = self.cur().offset();
        let mut marker = self.start();
        let mut left = self.parse_logical_and_pattern();
        while self.eat(TokenKind::PipePipe).is_some() {
            let right = self.parse_logical_and_pattern();
            let span = self.span_from(start);
            left = Pattern::LogicalOr {
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
            let completed = marker.complete(self, NodeKind::LogicalOrPattern);
            if !self.at(TokenKind::PipePipe) {
                return left;
            }
            marker = completed.precede(self);
        }
        marker.abandon(self);
        left
    }

    fn parse_logical_and_pattern(&mut self) -> Pattern {
        let start = self.cur().offset();
        let mut marker = self.start();
        let mut left = self.parse_relational_pattern();
        while self.eat(TokenKind::AmpAmp).is_some() {
            let right = self.parse_relational_pattern();
            let span = self.span_from(start);
            left = Pattern::LogicalAnd {
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
            let completed = marker.complete(self, NodeKind::LogicalAndPattern);
            if !self.at(TokenKind::AmpAmp) {
                return left;
            }
            marker = completed.precede(self);
        }
        marker.abandon(self);
        left
    }

    fn parse_relational_pattern(&mut self) -> Pattern {
        let start = self.cur().offset();
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
            let marker = self.start();
            self.advance();
            // The operand is a `bitwiseOrExpression`: parsing at this tier keeps
            // `&&`/`||` at the pattern level so `> 3 && < 5` is a logical-and
            // pattern rather than the operand swallowing the `&&`.
            let value = self.parse_bitwise_or();
            marker.complete(self, NodeKind::RelationalPattern);
            return Pattern::Relational {
                op,
                value,
                span: self.span_from(start),
            };
        }
        self.parse_postfix_pattern()
    }

    // Balanced type arguments followed by a collection distinguish `<` from comparison.
    fn typed_collection_ahead(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::Lt);
        self.matching_angle_close(self.pos, |_| true, Some(513))
            .is_some_and(|close| {
                matches!(
                    self.kind_at(close + 1),
                    TokenKind::LBracket | TokenKind::LBrace
                )
            })
    }

    fn parse_postfix_pattern(&mut self) -> Pattern {
        let start = self.cur().offset();
        let mut marker = self.start();
        let mut inner = self.parse_primary_pattern();
        loop {
            if self.eat(TokenKind::Qmark).is_some() {
                let span = self.span_from(start);
                inner = Pattern::NullCheck {
                    inner: Box::new(inner),
                    span,
                };
                let completed = marker.complete(self, NodeKind::NullCheckPattern);
                if !self.at_any(&[TokenKind::Qmark, TokenKind::Bang, TokenKind::As]) {
                    return inner;
                }
                marker = completed.precede(self);
            } else if self.eat(TokenKind::Bang).is_some() {
                let span = self.span_from(start);
                inner = Pattern::NullAssert {
                    inner: Box::new(inner),
                    span,
                };
                let completed = marker.complete(self, NodeKind::NullAssertPattern);
                if !self.at_any(&[TokenKind::Qmark, TokenKind::Bang, TokenKind::As]) {
                    return inner;
                }
                marker = completed.precede(self);
            } else if self.eat(TokenKind::As).is_some() {
                let cast_type = self.parse_type();
                let span = self.span_from(start);
                inner = Pattern::Cast {
                    inner: Box::new(inner),
                    cast_type,
                    span,
                };
                let completed = marker.complete(self, NodeKind::CastPattern);
                if !self.at_any(&[TokenKind::Qmark, TokenKind::Bang, TokenKind::As]) {
                    return inner;
                }
                marker = completed.precede(self);
            } else {
                break;
            }
        }
        marker.abandon(self);
        inner
    }

    fn parse_primary_pattern(&mut self) -> Pattern {
        let marker = self.start();
        let value_marker = self.start();
        let pattern = self.parse_primary_pattern_inner();
        if matches!(
            &pattern,
            Pattern::Const(ConstPattern {
                value: ConstPatternValue::Expr(_),
                ..
            })
        ) {
            value_marker.complete(self, NodeKind::ConstPatternExpr);
        } else {
            value_marker.abandon(self);
        }
        let kind = match &pattern {
            Pattern::Wildcard { .. } => NodeKind::WildcardPattern,
            Pattern::Variable { .. } => NodeKind::VariablePattern,
            Pattern::Literal(_) => NodeKind::LiteralPattern,
            Pattern::Const(_) => NodeKind::ConstPattern,
            Pattern::List(_) => NodeKind::ListPattern,
            Pattern::Record(_) => NodeKind::RecordPattern,
            Pattern::Map(_) => NodeKind::MapPattern,
            Pattern::Object(_) => NodeKind::ObjectPattern,
            Pattern::ParenPattern { .. } => NodeKind::ParenPattern,
            Pattern::Error { .. } => NodeKind::ErrorPattern,
            _ => unreachable!("primary pattern cannot produce an infix or postfix pattern"),
        };
        marker.complete(self, kind);
        pattern
    }

    fn parse_primary_pattern_inner(&mut self) -> Pattern {
        let start = self.cur().offset();

        // Dot-shorthand constant pattern (Dart 3.x): `.name` / `.new` — an
        // enum/static-member shorthand used as a pattern (`case .build => ...`).
        if self.at(TokenKind::Dot) {
            let expr = self.parse_dot_shorthand(false, start);
            return Pattern::Const(ConstPattern {
                value: ConstPatternValue::Expr(Box::new(expr)),
                span: self.span_from(start),
            });
        }

        // Record-type-led typed variable/wildcard: `(int, int)? b`. A `(...)`
        // (optionally nullable) that is directly followed by a binding name is a
        // record TYPE annotation, not a record pattern.
        if self.at(TokenKind::LParen) && self.record_type_var_pattern_ahead() {
            let ty = self.parse_type();
            if self.eat_wildcard() {
                return Pattern::Wildcard {
                    type_: Some(ty),
                    span: self.span_from(start),
                };
            }
            let name = self.expect_ident();
            return Pattern::Variable {
                type_: Some(ty),
                name,
                span: self.span_from(start),
            };
        }

        if self.at(TokenKind::LParen) {
            return self.parse_paren_or_record_pattern(start);
        }

        if self.at(TokenKind::LBracket) {
            return self.parse_list_pattern(start, None);
        }

        if self.at(TokenKind::LBrace) {
            return self.parse_map_pattern(start, Vec::new());
        }

        // Typed collection pattern: `<int>[a, b]` or `<String, int>{...}`.
        if self.at(TokenKind::Lt) {
            let type_args = self.parse_type_args();
            if self.at(TokenKind::LBracket) {
                if type_args.len() != 1 {
                    self.error(format!(
                        "list pattern expects exactly one type argument, got {}",
                        type_args.len()
                    ));
                }
                return self.parse_list_pattern(start, type_args.into_iter().next());
            }
            if self.at(TokenKind::LBrace) {
                if type_args.len() != 2 {
                    self.error(format!(
                        "map pattern expects exactly two type arguments, got {}",
                        type_args.len()
                    ));
                }
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

        if self.eat(TokenKind::Null).is_some() {
            return Pattern::Literal(LiteralPattern {
                value: LiteralPatternValue::Null,
                span: self.span_from(start),
            });
        }

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

        // String literal. Adjacent string literals are implicitly concatenated
        // into one constant string, legal wherever a constant pattern appears.
        // A member access on it (`"Lax".length`) is a constant expression the
        // front end accepts as a constant pattern too.
        if self.at(TokenKind::StringLit) {
            let mut field_marker = self.start();
            let node = self.parse_adjacent_strings();
            if !(self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like()) {
                field_marker.abandon(self);
                return Pattern::Literal(LiteralPattern {
                    value: LiteralPatternValue::String(node),
                    span: self.span_from(start),
                });
            }
            let mut expr = Expr::StringLit(node);
            while self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like() {
                self.advance(); // .
                let field = self.expect_ident();
                expr = Expr::Field {
                    object: Box::new(expr),
                    field,
                    is_null_safe: false,
                    span: self.span_from(start),
                };
                let completed = field_marker.complete(self, NodeKind::FieldExpr);
                if !(self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like()) {
                    break;
                }
                field_marker = completed.precede(self);
            }
            return Pattern::Const(ConstPattern {
                value: ConstPatternValue::Expr(Box::new(expr)),
                span: self.span_from(start),
            });
        }

        // Symbol literal (`#name`, `#foo.bar`, `#+`) — a constant expression, so
        // it is a constant pattern in refutable positions (`case #add:`).
        if self.at(TokenKind::Hash) {
            let expr = self.parse_symbol_literal(start);
            return Pattern::Const(ConstPattern {
                value: ConstPatternValue::Expr(Box::new(expr)),
                span: self.span_from(start),
            });
        }

        // const keyword → const constructor or const value pattern
        if self.at(TokenKind::Const) {
            return self.parse_const_pattern(start);
        }

        if self.eat_wildcard() {
            return Pattern::Wildcard {
                type_: None,
                span: self.span_from(start),
            };
        }

        // var / final → variable pattern  (var x, final x, final Type x)
        if self.at(TokenKind::Var) {
            self.advance();
            if self.eat_wildcard() {
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
            let var_type = self.try_parse_type_followed_by(|kind| {
                kind.is_identifier_like() && !matches!(kind, TokenKind::When | TokenKind::As)
            });
            if self.eat_wildcard() {
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

    // A binding after a balanced record type distinguishes it from a record pattern.
    fn record_type_var_pattern_ahead(&self) -> bool {
        debug_assert_eq!(self.cur().kind, TokenKind::LParen);
        let Some(close) = self.matching_close(self.pos, Brackets::Mixed, |_| false, None) else {
            return false;
        };
        let mut next = close + 1;
        if self.kind_at(next) == TokenKind::Qmark {
            next += 1;
        }
        let kind = self.kind_at(next);
        kind.is_identifier_like() && !matches!(kind, TokenKind::As | TokenKind::When)
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
            || (self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon)
        {
            return self.parse_record_pattern_body(start);
        }

        let first_marker = self.start();
        let first = self.parse_pattern();

        if self.at(TokenKind::Comma) {
            first_marker.complete(self, NodeKind::RecordPatternField);
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
            first_marker.abandon(self);
            // Single element → parenthesised pattern
            if !self.at(TokenKind::RParen) {
                self.pattern_recovery_span();
            }
            let close = self.expect(TokenKind::RParen);
            Pattern::ParenPattern {
                inner: Box::new(first),
                span: Span::new(start, close.offset() + close.len()),
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
        let fs = self.cur().offset();
        let marker = self.start();
        if self.at(TokenKind::Colon) {
            self.advance(); // :
            // `:name` (also `:var name` / `:final name`) always binds a variable.
            let pattern = self.parse_binding_pattern();
            let name = binding_pattern_name(&pattern);
            marker.complete(self, NodeKind::RecordPatternField);
            return RecordPatternField {
                name,
                pattern,
                span: self.span_from(fs),
            };
        }
        let name = if self.cur().kind.is_identifier_like() && self.peek(1).kind == TokenKind::Colon
        {
            let n = self.expect_ident();
            self.advance(); // :
            Some(n)
        } else {
            None
        };
        let pattern = self.parse_pattern();
        marker.complete(self, NodeKind::RecordPatternField);
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
            let es = self.cur().offset();
            if self.at(TokenKind::DotDotDot) {
                let marker = self.start();
                self.advance();
                let rest_pat = if !self.at(TokenKind::RBracket) && !self.at(TokenKind::Comma) {
                    Some(self.parse_pattern())
                } else {
                    None
                };
                marker.complete(self, NodeKind::RestPattern);
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
            let es = self.cur().offset();
            let marker = self.start();
            let key = self.parse_expr();
            self.expect(TokenKind::Colon);
            let pattern = self.parse_pattern();
            marker.complete(self, NodeKind::MapPatternEntry);
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
        // Parenthesized constant expression `const (1 + 2)`, or a const record
        // `const (1, 2)`. Handled here rather than via `parse_primary`, which would
        // read `(...)` as a record type. `parse_paren_or_record` yields the inner
        // expression for a single element and a const `Expr::Record` when commas
        // make it a record shape.
        if self.peek(1).kind == TokenKind::LParen {
            let marker = self.start();
            self.advance(); // const
            let inner = self.parse_paren_or_record_inner(true, start, marker);
            return Pattern::Const(ConstPattern {
                value: ConstPatternValue::Expr(Box::new(inner)),
                span: self.span_from(start),
            });
        }

        // Const constructors and collections are expressions; dotted references are names.
        let is_expr_form = match self.peek(1).kind {
            TokenKind::LBracket | TokenKind::LBrace | TokenKind::Lt => true,
            _ if self.peek(1).kind.is_identifier_like() => {
                // A const constructor has `(` / `<` after the (dotted) name; a
                // bare constant reference does not.
                let mut i = 2;
                while self.peek(i).kind == TokenKind::Dot
                    && self.peek(i + 1).kind.is_identifier_like()
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
                value: ConstPatternValue::Expr(Box::new(expr)),
                span: self.span_from(start),
            });
        }

        // Bare dotted-name constant reference: `const Foo`, `const Foo.bar`.
        self.advance(); // const
        let mut name = Vec::new();
        if self.cur().kind.is_identifier_like() {
            name.push(self.expect_ident());
            while self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like() {
                self.advance(); // .
                name.push(self.expect_ident());
            }
        }
        Pattern::Const(ConstPattern {
            value: ConstPatternValue::Name(name),
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
                let fs = self.cur().offset();
                let marker = self.start();
                // Dart 3 colon-shorthand: `:field` binds a variable whose getter
                // name is the bound name (also `:var x` / `:final x`). Parse in
                // binding mode so `field` becomes a variable, not a constant
                // reference, in refutable contexts like `case Foo(:field)`.
                if self.at(TokenKind::Colon) {
                    self.advance(); // :
                    let inner = self.parse_binding_pattern();
                    let field_name = binding_pattern_name(&inner).unwrap_or_else(|| {
                        self.pattern_recovery_span();
                        Identifier::new("<shorthand>", self.cur_span())
                    });
                    marker.complete(self, NodeKind::ObjectPatternField);
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
                let name = if self.cur().kind.is_identifier_like()
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
                if name.is_none() {
                    self.pattern_recovery_span();
                }
                marker.complete(self, NodeKind::ObjectPatternField);
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

        if self.eat_wildcard() {
            return Pattern::Wildcard {
                type_: Some(ty),
                span: self.span_from(start),
            };
        }

        // Typed variable: Type name. `when` here introduces a case guard
        // (`case State.a when c`) and `as` a cast pattern (`value as int`), not a
        // variable name, so neither must be consumed as the binding.
        if self.cur().kind.is_identifier_like() && !self.at_any(&[TokenKind::When, TokenKind::As]) {
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
            DartType::Named(nt) => {
                if self.ctx.pattern_binding
                    && nt.segments.len() == 1
                    && nt.type_args.is_empty()
                    && !nt.is_nullable
                {
                    let name = nt.segments.into_iter().next().unwrap();
                    Pattern::Variable {
                        type_: None,
                        name,
                        span: self.span_from(start),
                    }
                } else {
                    Pattern::Const(ConstPattern {
                        value: ConstPatternValue::Name(nt.segments),
                        span: self.span_from(start),
                    })
                }
            }
            // `dynamic` is a built-in DartType, but its bare pattern is a type
            // literal just like a named type, not a recovery/error pattern.
            DartType::Dynamic { span } => Pattern::Const(ConstPattern {
                value: ConstPatternValue::Name(vec![Identifier::new("dynamic", span)]),
                span: self.span_from(start),
            }),
            _ => Pattern::Error {
                span: self.span_from(start),
            },
        }
    }

    fn pattern_recovery_span(&mut self) {
        self.events
            .push(falcon_syntax::cst::Event::RecoverySpan(self.cur_span()));
    }

    fn eat_wildcard(&mut self) -> bool {
        if self.at(TokenKind::Ident) && self.cur_text() == "_" {
            self.advance();
            true
        } else {
            false
        }
    }
}

/// The bound name of a `:name`-shorthand record or object field, taken from the
/// inner variable pattern, possibly wrapped in a cast, null-check, or
/// null-assert pattern (`:final x?`, `:x!`, `:var x as T`).
fn binding_pattern_name(pattern: &Pattern) -> Option<Identifier> {
    match pattern {
        Pattern::Variable { name, .. } => Some(name.clone()),
        Pattern::Cast { inner, .. }
        | Pattern::NullCheck { inner, .. }
        | Pattern::NullAssert { inner, .. } => binding_pattern_name(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_syntax::{LanguageVersion, cst::SyntaxTree};

    #[test]
    fn pattern_views_match_legacy_debug() {
        for source in [
            "_",
            "int _",
            "var x",
            "final int x",
            "null",
            "true",
            "false",
            "1",
            "1.5",
            "-1",
            "-1.5",
            "'a' 'b'",
            "'a'.length",
            ".foo",
            "#foo",
            "const Foo",
            "const Foo.bar",
            "const (foo)",
            "const (1, 2)",
            "const Foo()",
            "const [1]",
            "[var a, ...var rest]",
            "<int>[1, ...]",
            "{'a': var a, ...}",
            "<String, int>{'a': 1}",
            "()",
            "(var a)",
            "(var a, var b)",
            "(:x, named: var y)",
            "(:final x?, :var y as int)",
            "Foo(:x, named: var y)",
            "Foo(:1)",
            "Foo(var x)",
            "> 3 && < 5 || == 7",
            "var x?! as int",
            "(1:",
            "1 || 2 || 3",
            "1 && 2 && 3",
            "'a'.length.isEven",
            "dynamic?",
            "( , x)",
            "Foo(,)",
            "List<int>",
            "p.Foo",
            "Foo?",
            "(a, b)",
            "[a, ...rest]",
        ] {
            for binding in [false, true] {
                let syntax = super::super::lex_syntax(source);
                let mut parser = Parser::new(&syntax, source, LanguageVersion::CURRENT);
                let root = parser.start();
                let legacy = if binding {
                    parser.parse_binding_pattern()
                } else {
                    parser.parse_pattern()
                };
                root.complete(&mut parser, NodeKind::Program);
                let tokens = super::super::lexical::cst_tokens(&syntax, source);
                let tree = SyntaxTree::build(source, &tokens, parser.events);
                let pattern = tree
                    .root(source)
                    .children()
                    .find_map(falcon_syntax::cst::Pattern::cast)
                    .expect("pattern parser emits a pattern node");
                assert_eq!(format!("{legacy:#?}"), format!("{pattern:#?}"), "{source}");
                assert!(tree.is_lossless(source), "{source}");
            }
        }
    }
}
