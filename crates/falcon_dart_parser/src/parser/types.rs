use falcon_syntax::ast::*;
use falcon_syntax::language_version::Feature;
use falcon_syntax::token::TokenKind;

use super::Parser;

impl<'src> Parser<'src> {
    pub(super) fn parse_type(&mut self) -> DartType {
        let start = self.cur().offset();

        if matches!(self.cur().kind, TokenKind::Void | TokenKind::Dynamic) {
            let kind = self.advance().kind;
            let builtin = |span| match kind {
                TokenKind::Void => DartType::Void { span },
                _ => DartType::Dynamic { span },
            };
            if self.at_function_type_start() {
                return self.parse_function_type(Some(builtin(self.span_from(start))), start);
            }
            self.eat_type_qmark();
            return builtin(self.span_from(start));
        }

        // Function type starting with `Function(` or `Function<T>(`
        if self.at_function_type_start() {
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
            while self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like() {
                self.advance(); // .
                segments.push(self.expect_ident());
            }

            let type_args = self.parse_type_args();

            // A `?` here belongs to this named type — including when it is the
            // (nullable) return type of a following function type, e.g.
            // `String? Function(...)`.
            let is_nullable = self.eat_type_qmark();

            let named = DartType::Named(NamedType {
                segments,
                type_args,
                is_nullable,
                span: self.span_from(start),
            });
            // Could be `Function(...)` / `Function<T>(...)` after a (possibly
            // nullable) return type.
            if self.at_function_type_start() {
                return self.parse_function_type(Some(named), start);
            }
            return named;
        }

        // Fallback: produce dynamic
        self.error(format!("expected type, got {:?}", self.cur().kind));
        DartType::Dynamic {
            span: self.span_from(start),
        }
    }

    pub(super) fn is_type_start(&self) -> bool {
        self.cur().kind.is_identifier_like()
            || matches!(self.cur().kind, TokenKind::Void | TokenKind::LParen)
    }

    /// `Function(` or `Function<` — the start of an (optionally generic)
    /// inline function type, distinct from a bare type named `Function`.
    fn at_function_type_start(&self) -> bool {
        self.at(TokenKind::Function)
            && matches!(self.peek(1).kind, TokenKind::LParen | TokenKind::Lt)
    }

    fn parse_function_type(&mut self, return_type: Option<DartType>, start: usize) -> DartType {
        let keyword = self.pos;
        self.advance(); // Function keyword
        let type_params = self.parse_type_params();
        self.expect(TokenKind::LParen);
        let params = self.with_ctx(
            |ctx| ctx.suppress_conditional_qmark = false,
            Self::parse_function_type_params,
        );
        self.expect(TokenKind::RParen);
        let is_nullable = self.eat_type_qmark();
        // Set after the parameters, so the outermost function type — which
        // completes last, including a curried suffix — has the final say.
        self.last_function_keyword = Some(keyword);
        let span = self.span_from(start);
        let fn_type = DartType::Function(Box::new(FunctionType {
            return_type: return_type.map(Box::new),
            type_params,
            params,
            is_nullable,
            span,
        }));
        // Curried function type: the function type just parsed may itself be the
        // return type of another, `int Function() Function()` being a function
        // returning `int Function()`. Fold left-to-right so the inner type nests as
        // the return type of the outer.
        if self.at_function_type_start() {
            return self.parse_function_type(Some(fn_type), start);
        }
        fn_type
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
            let annotations = self.parse_annotations();
            let explicit_required = self.eat(TokenKind::Required).is_some();
            let is_required = explicit_required || (!in_named && !in_optional);
            let _ = self
                .eat(TokenKind::Final)
                .or_else(|| self.eat(TokenKind::Covariant));
            let param_type = self.parse_type();
            let name = self.parse_ident();
            params.push(FunctionTypeParam {
                annotations,
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
        let mut positional_names = Vec::new();
        let mut named = Vec::new();

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::LBrace) {
                self.advance();
                while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                    if self.at(TokenKind::At) {
                        self.parse_annotations();
                    }
                    let field_type = self.parse_type();
                    let name = self.expect_ident();
                    named.push(NamedRecordField { name, field_type });
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.eat(TokenKind::RBrace);
            } else {
                if self.at(TokenKind::At) {
                    self.parse_annotations();
                }
                positional.push(self.parse_type());
                let name = if self.cur().kind.is_identifier_like()
                    && matches!(self.peek(1).kind, TokenKind::Comma | TokenKind::RParen)
                {
                    self.parse_ident()
                } else {
                    None
                };
                positional_names.push(name);
            }
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        let is_nullable = self.eat_type_qmark();
        let record = DartType::Record(RecordType {
            positional,
            positional_names,
            named,
            is_nullable,
            span: self.span_from(start),
        });
        // A record type may itself be the return type of a function type, e.g.
        // `(int, int) Function()` — check for the `Function(...)` suffix here so it
        // parses in specialized type positions (typedef RHS, field/param types).
        if self.at_function_type_start() {
            return self.parse_function_type(Some(record), start);
        }
        record
    }

    pub(super) fn parse_type_params(&mut self) -> Vec<TypeParam> {
        if !self.at(TokenKind::Lt) {
            return Vec::new();
        }
        self.advance(); // <
        let mut params = Vec::new();
        while !self.at(TokenKind::Gt) && !self.at(TokenKind::GtGt) && !self.at(TokenKind::Eof) {
            let start = self.cur().offset();
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

    pub(super) fn parse_formal_param_list(&mut self) -> FormalParamList {
        let start = self.cur().offset();
        self.expect(TokenKind::LParen);

        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at_any(&[TokenKind::LBracket, TokenKind::LBrace]) {
                let named = self.advance().kind == TokenKind::LBrace;
                let close = if named {
                    TokenKind::RBrace
                } else {
                    TokenKind::RBracket
                };
                let kind = if named {
                    ParamKind::Named { required: false }
                } else {
                    ParamKind::OptionalPositional
                };
                while !self.at(close) && !self.at(TokenKind::Eof) {
                    params.push(self.parse_formal_param(kind));
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.eat(close);
                // An optional/named group is the last in the list: `(a, [b], {c})`
                // is a syntax error.
                break;
            } else {
                params.push(self.parse_formal_param(ParamKind::Positional));
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }

        self.expect(TokenKind::RParen);
        FormalParamList {
            params,
            span: self.span_from(start),
        }
    }

    fn parse_formal_param(&mut self, mut kind: ParamKind) -> FormalParam {
        let start = self.cur().offset();
        let mut annotations = Vec::new();
        while self.at(TokenKind::At) {
            annotations.extend(self.parse_annotations());
        }
        let is_required = self.eat(TokenKind::Required).is_some();
        if let ParamKind::Named { required } = &mut kind {
            *required = is_required;
        }
        let is_covariant = self.eat(TokenKind::Covariant).is_some();
        let is_final = self.eat(TokenKind::Final).is_some();
        // `var` marks a mutable parameter (declaring in a primary constructor);
        // like `final`, it may be followed by a type (`C(var int x)`).
        let is_var = !is_final && self.eat(TokenKind::Var).is_some();

        // A `this.field` / `super.field` formal may be preceded by a type
        // (`C(int this.x)`, `C(int super.x)`), so parse an optional leading type
        // before deciding.
        let mut initializing = None;
        let leading = self.try_parse_type_followed_by(|kind| {
            kind.is_identifier_like() || matches!(kind, TokenKind::This | TokenKind::Super)
        });
        let (param_type, name) = if self.at(TokenKind::This) && self.peek(1).kind == TokenKind::Dot
        {
            initializing = Some(InitializingFormal::This);
            self.advance(); // this
            self.advance(); // .
            (leading, self.expect_ident())
        } else if self.at(TokenKind::Super) && self.peek(1).kind == TokenKind::Dot {
            initializing = Some(InitializingFormal::Super);
            self.advance(); // super
            self.advance(); // .
            (leading, self.expect_ident())
        } else if let Some(function_named) = leading
            .is_none()
            .then(|| self.function_named_formal())
            .flatten()
        {
            function_named
        } else {
            (leading, self.expect_ident())
        };
        if matches!(kind, ParamKind::Named { .. })
            && initializing == Some(InitializingFormal::This)
            && name.name.starts_with('_')
            && !self.version.supports(Feature::PrivateNamedParameters)
        {
            self.error_at(
                name.span.start,
                "This requires the 'private-named-parameters' language feature",
            );
        }

        // Generic old-style function-typed formal: `name<T>(params)`. Dart allows
        // type parameters on a function-typed formal parameter (unlike inside a
        // generic function TYPE, which the caller keeps rejecting).
        let type_params = self.parse_type_params();
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
            kind,
            is_covariant,
            is_final,
            is_var,
            initializing,
            param_type,
            name,
            default_value,
            type_params,
            function_params,
            span: self.span_from(start),
        }
    }

    /// An old-style function-typed formal whose *name* is the built-in
    /// identifier `Function`: `void map(void Function({int? e}))`. The input
    /// reads as an unnamed function type, so when a full type parse ends in a
    /// `Function(...)` suffix that no parameter name follows, re-read that
    /// suffix as the parameter's name and parameter list, keeping the cursor
    /// before `Function`. Returns the return type and the `Function` name.
    fn function_named_formal(&mut self) -> Option<(Option<DartType>, Identifier)> {
        if !self.is_type_start() {
            return None;
        }
        self.speculate(|parser| {
            parser.last_function_keyword = None;
            let ty = parser.parse_type();
            let keyword = parser.last_function_keyword?;
            let ends_param = matches!(
                parser.cur().kind,
                TokenKind::Comma
                    | TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::RBrace
                    | TokenKind::Eq
                    | TokenKind::Colon
            );
            let DartType::Function(function) = ty else {
                return None;
            };
            if !ends_param {
                return None;
            }
            parser.rewind_to(keyword);
            let name_tok = parser.advance(); // Function
            Some((
                function.return_type.map(|ret| *ret),
                parser.tok_ident(&name_tok),
            ))
        })
    }
}
