use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::Parser;

impl<'src> Parser<'src> {
    // ── DartType ──────────────────────────────────────────────────────────────

    pub(super) fn parse_type(&mut self) -> DartType {
        let start = self.cur().offset;

        // void
        if self.at(TokenKind::Void) {
            self.advance();
            // `void Function(...)` is a function type returning void
            if self.at(TokenKind::Function) && self.peek(1).kind == TokenKind::LParen {
                let ret = DartType::Void {
                    span: self.span_from(start),
                };
                return self.parse_function_type(Some(ret), start);
            }
            let nullable = self.eat_type_qmark();
            let span = self.span_from(start);
            return if nullable {
                // void? is technically invalid Dart but we parse it gracefully
                DartType::Named(NamedType {
                    segments: vec![Identifier::new("void", span.clone())],
                    type_args: vec![],
                    is_nullable: true,
                    span,
                })
            } else {
                DartType::Void { span }
            };
        }

        // dynamic
        if self.at(TokenKind::Dynamic) {
            self.advance();
            let is_nullable = self.eat_type_qmark();
            let span = self.span_from(start);
            return if is_nullable {
                DartType::Named(NamedType {
                    segments: vec![Identifier::new("dynamic", span.clone())],
                    type_args: vec![],
                    is_nullable: true,
                    span,
                })
            } else {
                DartType::Dynamic { span }
            };
        }

        // Function type starting with `Function(`
        if self.at(TokenKind::Function) && self.peek(1).kind == TokenKind::LParen {
            return self.parse_function_type(None, start);
        }

        // Record type: (type, name: type) — must precede is_type_start() since LParen is in it
        if self.at(TokenKind::LParen) {
            return self.parse_record_type(start);
        }

        // Named type: Identifier [. Identifier] [< ... >] [?]
        if self.is_type_start() {
            let first = self.expect_ident();
            let mut segments = vec![first];

            // Qualified: Foo.Bar
            while self.at(TokenKind::Dot) && self.is_ident_like_at(1) {
                self.advance(); // .
                segments.push(self.expect_ident());
            }

            let type_args = if self.at(TokenKind::Lt) {
                self.parse_type_args()
            } else {
                Vec::new()
            };

            // A `?` here belongs to this named type — including when it is the
            // (nullable) return type of a following function type, e.g.
            // `String? Function(...)`.
            let is_nullable = self.eat_type_qmark();

            // Could be `Function(...)` after a (possibly nullable) return type.
            if self.at(TokenKind::Function) && self.peek(1).kind == TokenKind::LParen {
                let ret = DartType::Named(NamedType {
                    segments,
                    type_args,
                    is_nullable,
                    span: self.span_from(start),
                });
                return self.parse_function_type(Some(ret), start);
            }

            let span = self.span_from(start);
            return DartType::Named(NamedType {
                segments,
                type_args,
                is_nullable,
                span,
            });
        }

        // Record type: (type, name: type)
        if self.at(TokenKind::LParen) {
            return self.parse_record_type(start);
        }

        // Fallback: produce dynamic
        self.error(format!("expected type, got {:?}", self.cur().kind));
        DartType::Dynamic {
            span: self.span_from(start),
        }
    }

    pub(super) fn is_type_start(&self) -> bool {
        self.is_ident_like() || matches!(self.cur().kind, TokenKind::Void | TokenKind::LParen)
    }

    pub(super) fn is_ident_like_at(&self, offset: usize) -> bool {
        self.is_ident_like_kind(&self.peek(offset).kind)
    }

    pub(super) fn is_ident_like_kind(&self, kind: &TokenKind) -> bool {
        use TokenKind::*;
        matches!(
            kind,
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

    fn parse_function_type(&mut self, return_type: Option<DartType>, start: usize) -> DartType {
        self.advance(); // Function keyword
        let type_params = if self.at(TokenKind::Lt) {
            self.parse_type_params()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LParen);
        let params = self.parse_function_type_params();
        self.expect(TokenKind::RParen);
        let is_nullable = self.eat_type_qmark();
        let span = self.span_from(start);
        DartType::Function(Box::new(FunctionType {
            return_type: return_type.map(Box::new),
            type_params,
            params,
            is_nullable,
            span,
        }))
    }

    fn parse_function_type_params(&mut self) -> Vec<FunctionTypeParam> {
        let mut params = Vec::new();
        let mut in_named = false;
        let mut in_optional = false;

        while !self.at(TokenKind::RParen)
            && !self.at(TokenKind::RBrace)
            && !self.at(TokenKind::RBracket)
            && !self.at(TokenKind::Eof)
        {
            // The named `{...}` / optional-positional `[...]` section may follow
            // leading required-positional params, so detect it inside the loop.
            if !in_named && !in_optional {
                if self.at(TokenKind::LBrace) {
                    self.advance();
                    in_named = true;
                    continue;
                } else if self.at(TokenKind::LBracket) {
                    self.advance();
                    in_optional = true;
                    continue;
                }
            }
            let is_required = self.eat(TokenKind::Required).is_some();
            let _ = self
                .eat(TokenKind::Final)
                .or_else(|| self.eat(TokenKind::Covariant));
            let param_type = self.parse_type();
            // Optional name
            let name = if self.is_ident_like() {
                self.parse_ident()
            } else {
                None
            };
            params.push(FunctionTypeParam {
                name,
                param_type,
                is_required,
                is_named: in_named,
            });
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        if in_named {
            self.eat(TokenKind::RBrace);
        } else if in_optional {
            self.eat(TokenKind::RBracket);
        }
        params
    }

    fn parse_record_type(&mut self, start: usize) -> DartType {
        self.advance(); // (
        let mut positional = Vec::new();
        let mut named = Vec::new();

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::LBrace) {
                self.advance();
                while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                    let field_type = self.parse_type();
                    let name = self.expect_ident();
                    named.push(NamedRecordField { name, field_type });
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.eat(TokenKind::RBrace);
            } else {
                positional.push(self.parse_type());
                // Skip optional name
                if (self.is_ident_like()
                    && !self.at(TokenKind::Comma)
                    && self.peek(1).kind == TokenKind::Comma
                    || self.peek(1).kind == TokenKind::RParen)
                    && self.is_ident_like()
                {
                    self.advance();
                }
            }
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        let is_nullable = self.eat_type_qmark();
        DartType::Record(RecordType {
            positional,
            named,
            is_nullable,
            span: self.span_from(start),
        })
    }

    // ── Type parameters <T extends Bound, U> ──────────────────────────────────

    pub(super) fn parse_type_params(&mut self) -> Vec<TypeParam> {
        if !self.at(TokenKind::Lt) {
            return Vec::new();
        }
        self.advance(); // <
        let mut params = Vec::new();
        while !self.at(TokenKind::Gt) && !self.at(TokenKind::GtGt) && !self.at(TokenKind::Eof) {
            let start = self.cur().offset;
            let annotations = self.parse_annotations();
            let name = self.expect_ident();
            let bound = if self.eat(TokenKind::Extends).is_some() {
                Some(self.parse_type())
            } else {
                None
            };
            params.push(TypeParam {
                annotations,
                name,
                bound,
                span: self.span_from(start),
            });
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        if self.at(TokenKind::GtGt) {
            self.advance();
        } else {
            self.eat(TokenKind::Gt);
        }
        params
    }

    // ── Formal parameter list ─────────────────────────────────────────────────

    pub(super) fn parse_formal_param_list(&mut self) -> FormalParamList {
        let start = self.cur().offset;
        self.expect(TokenKind::LParen);

        let mut positional = Vec::new();
        let mut optional_positional = Vec::new();
        let mut named = Vec::new();

        if self.at(TokenKind::LBracket) {
            self.advance();
            while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                optional_positional.push(self.parse_formal_param(false));
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.eat(TokenKind::RBracket);
        } else if self.at(TokenKind::LBrace) {
            self.advance();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                named.push(self.parse_formal_param(true));
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.eat(TokenKind::RBrace);
        } else {
            while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
                if self.at(TokenKind::LBracket) {
                    self.advance();
                    while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                        optional_positional.push(self.parse_formal_param(false));
                        if self.eat(TokenKind::Comma).is_none() {
                            break;
                        }
                    }
                    self.eat(TokenKind::RBracket);
                    self.eat(TokenKind::Comma);
                } else if self.at(TokenKind::LBrace) {
                    self.advance();
                    while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                        named.push(self.parse_formal_param(true));
                        if self.eat(TokenKind::Comma).is_none() {
                            break;
                        }
                    }
                    self.eat(TokenKind::RBrace);
                    break;
                } else {
                    positional.push(self.parse_formal_param(false));
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
            }
        }

        self.expect(TokenKind::RParen);
        FormalParamList {
            positional,
            optional_positional,
            named,
            span: self.span_from(start),
        }
    }

    fn parse_formal_param(&mut self, is_named: bool) -> FormalParam {
        let start = self.cur().offset;
        let mut annotations = Vec::new();
        while self.at(TokenKind::At) {
            annotations.extend(self.parse_annotations());
        }
        let is_required = self.eat(TokenKind::Required).is_some();
        let is_covariant = self.eat(TokenKind::Covariant).is_some();
        let is_final = self.eat(TokenKind::Final).is_some();
        // `var` marks an untyped mutable parameter; consume it so the following
        // identifier is read as the name rather than a type.
        let is_var = self.eat(TokenKind::Var).is_some();

        // A `this.field` / `super.field` formal may be preceded by a type
        // (`C(int this.x)`, `C(int super.x)`), so parse an optional leading type
        // before deciding.
        let mut is_field = false;
        let mut is_super = false;
        let (param_type, name) = if is_var {
            // `var name` — untyped, the next token is the parameter name.
            (None, self.expect_ident())
        } else {
            let saved_pos = self.pos;
            let leading = if !self.at(TokenKind::This)
                && !self.at(TokenKind::Super)
                && self.is_type_start()
            {
                let ty = self.parse_type();
                // Accept the type only if a name / `this` / `super` follows;
                // otherwise it was actually the bare parameter name.
                if self.is_ident_like() || self.at(TokenKind::This) || self.at(TokenKind::Super) {
                    Some(ty)
                } else {
                    self.pos = saved_pos;
                    None
                }
            } else {
                None
            };

            if self.at(TokenKind::This) && self.peek(1).kind == TokenKind::Dot {
                is_field = true;
                self.advance(); // this
                self.advance(); // .
                (leading, self.expect_ident())
            } else if self.at(TokenKind::Super) && self.peek(1).kind == TokenKind::Dot {
                is_super = true;
                self.advance(); // super
                self.advance(); // .
                (leading, self.expect_ident())
            } else {
                (leading, self.expect_ident())
            }
        };

        // function-typed param: name(params)
        let function_params = if self.at(TokenKind::LParen) {
            Some(self.parse_formal_param_list())
        } else {
            None
        };
        // Nullable old-style function-typed formal: `{int orElse()?}`.
        if function_params.is_some() {
            self.eat(TokenKind::Qmark);
        }

        let default_value = if self.at_any(&[TokenKind::Eq, TokenKind::Colon]) {
            self.advance();
            Some(self.parse_expr())
        } else {
            None
        };

        FormalParam {
            annotations,
            is_required: is_required || !is_named,
            is_covariant,
            is_final,
            is_field,
            is_super,
            param_type,
            name,
            default_value,
            function_params,
            span: self.span_from(start),
        }
    }
}
