use falcon_syntax::cst::{NodeKind, ParamKind};
use falcon_syntax::language_version::Feature;
use falcon_syntax::token::TokenKind;

use super::{CompletedMarker, Parser};

impl<'src> Parser<'src> {
    pub(super) fn parse_type(&mut self) -> CompletedMarker {
        self.parse_type_with_binding_name().0
    }

    /// Also reports whether the type is an unqualified, argument-free, nonnullable name.
    pub(super) fn parse_type_with_binding_name(&mut self) -> (CompletedMarker, bool) {
        if matches!(self.cur().kind, TokenKind::Void | TokenKind::Dynamic) {
            let marker = self.start();
            let kind = self.advance().kind;
            let node_kind = match kind {
                TokenKind::Void => NodeKind::VoidType,
                _ => NodeKind::DynamicType,
            };
            if self.at_function_type_start() {
                let completed = marker.complete(self, node_kind);
                return (self.parse_function_type(Some(completed)), false);
            }
            self.eat_type_qmark();
            return (marker.complete(self, node_kind), false);
        }

        // Function type starting with `Function(` or `Function<T>(`
        if self.at_function_type_start() {
            return (self.parse_function_type(None), false);
        }

        // Record type: (type, name: type) — must precede is_type_start() since LParen is in it
        if self.at(TokenKind::LParen) {
            return (self.parse_record_type(), false);
        }

        // Named type: Identifier [. Identifier] [< ... >] [?]
        if self.is_type_start() {
            let marker = self.start();
            self.expect_ident();
            let mut unqualified = true;

            // Qualified: Foo.Bar
            while self.at(TokenKind::Dot) && self.peek(1).kind.is_identifier_like() {
                unqualified = false;
                self.advance(); // .
                self.expect_ident();
            }

            let type_args = self.parse_type_args();

            // A `?` here belongs to this named type — including when it is the
            // (nullable) return type of a following function type, e.g.
            // `String? Function(...)`.
            let nullable = self.eat_type_qmark();
            let completed = marker.complete(self, NodeKind::NamedType);
            // Could be `Function(...)` / `Function<T>(...)` after a (possibly
            // nullable) return type.
            if self.at_function_type_start() {
                return (self.parse_function_type(Some(completed)), false);
            }
            return (completed, unqualified && type_args == 0 && !nullable);
        }

        // Fallback: produce dynamic
        self.error(format!("expected type, got {:?}", self.cur().kind));
        (self.missing(NodeKind::DynamicType), false)
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

    fn parse_function_type(&mut self, return_marker: Option<CompletedMarker>) -> CompletedMarker {
        let keyword = self.checkpoint();
        let marker = match return_marker {
            Some(completed) => completed.precede(self),
            None => self.start(),
        };
        self.advance(); // Function keyword
        self.parse_type_params();
        self.expect(TokenKind::LParen);
        self.with_ctx(
            |ctx| ctx.suppress_conditional_qmark = false,
            Self::parse_function_type_params,
        );
        self.expect(TokenKind::RParen);
        self.eat_type_qmark();
        // Set after the parameters, so the outermost function type — which
        // completes last, including a curried suffix — has the final say.
        self.last_function_keyword = Some(keyword);
        let completed = marker.complete(self, NodeKind::FunctionType);
        // Curried function type: the function type just parsed may itself be the
        // return type of another, `int Function() Function()` being a function
        // returning `int Function()`. Fold left-to-right so the inner type nests as
        // the return type of the outer.
        if self.at_function_type_start() {
            return self.parse_function_type(Some(completed));
        }
        completed
    }

    fn parse_function_type_params(&mut self) {
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
            let marker = self.start();
            if in_named {
                self.missing(NodeKind::NamedTypeParamFlag);
            } else if in_optional {
                self.missing(NodeKind::OptionalTypeParamFlag);
            }
            self.parse_annotations();
            self.eat(TokenKind::Required);
            let _ = self
                .eat(TokenKind::Final)
                .or_else(|| self.eat(TokenKind::Covariant));
            self.parse_type();
            self.parse_ident();
            marker.complete(self, NodeKind::FunctionTypeParam);
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
    }

    fn parse_record_type(&mut self) -> CompletedMarker {
        let marker = self.start();
        self.advance(); // (

        while !self.at(TokenKind::RParen) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::LBrace) {
                self.advance();
                while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                    let field_marker = self.start();
                    if self.at(TokenKind::At) {
                        self.parse_annotations();
                    }
                    self.parse_type();
                    self.expect_ident();
                    field_marker.complete(self, NodeKind::NamedRecordField);
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.eat(TokenKind::RBrace);
            } else {
                if self.at(TokenKind::At) {
                    self.parse_annotations();
                }
                self.parse_type();
                if self.cur().kind.is_identifier_like()
                    && matches!(self.peek(1).kind, TokenKind::Comma | TokenKind::RParen)
                {
                    self.parse_ident();
                }
            }
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RParen);
        self.eat_type_qmark();
        let completed = marker.complete(self, NodeKind::RecordType);
        // A record type may itself be the return type of a function type, e.g.
        // `(int, int) Function()` — check for the `Function(...)` suffix here so it
        // parses in specialized type positions (typedef RHS, field/param types).
        if self.at_function_type_start() {
            return self.parse_function_type(Some(completed));
        }
        completed
    }

    pub(super) fn parse_type_params(&mut self) -> usize {
        if !self.at(TokenKind::Lt) {
            return 0;
        }
        self.advance(); // <
        let mut count = 0;
        while !self.at(TokenKind::Gt) && !self.at(TokenKind::GtGt) && !self.at(TokenKind::Eof) {
            let marker = self.start();
            self.parse_annotations();
            self.expect_ident();
            if self.eat(TokenKind::Extends).is_some() {
                self.parse_type();
            }
            count += 1;
            marker.complete(self, NodeKind::TypeParam);
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        if self.at(TokenKind::GtGt) {
            self.advance();
        } else {
            self.eat(TokenKind::Gt);
        }
        count
    }

    pub(super) fn parse_formal_param_list(&mut self) -> CompletedMarker {
        let marker = self.start();
        self.expect(TokenKind::LParen);

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
                    self.parse_formal_param(kind);
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.eat(close);
                // An optional/named group is the last in the list: `(a, [b], {c})`
                // is a syntax error.
                break;
            } else {
                self.parse_formal_param(ParamKind::Positional);
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }

        self.expect(TokenKind::RParen);
        marker.complete(self, NodeKind::FormalParamList)
    }

    fn parse_formal_param(&mut self, kind: ParamKind) -> CompletedMarker {
        let marker = self.start();
        match kind {
            ParamKind::Named { .. } => {
                self.missing(NodeKind::NamedTypeParamFlag);
            }
            ParamKind::OptionalPositional => {
                self.missing(NodeKind::OptionalTypeParamFlag);
            }
            ParamKind::Positional => {}
        }
        while self.at(TokenKind::At) {
            self.parse_annotations();
        }
        self.eat(TokenKind::Required);
        self.eat(TokenKind::Covariant);
        let is_final = self.eat(TokenKind::Final).is_some();
        // `var` marks a mutable parameter (declaring in a primary constructor);
        // like `final`, it may be followed by a type (`C(var int x)`).
        if !is_final {
            self.eat(TokenKind::Var);
        }

        // A `this.field` / `super.field` formal may be preceded by a type
        // (`C(int this.x)`, `C(int super.x)`), so parse an optional leading type
        // before deciding.
        let mut initializing_this = false;
        let leading = self.try_parse_type_followed_by(|kind| {
            kind.is_identifier_like() || matches!(kind, TokenKind::This | TokenKind::Super)
        });
        let name = if self.at(TokenKind::This) && self.peek(1).kind == TokenKind::Dot {
            initializing_this = true;
            self.advance(); // this
            self.advance(); // .
            self.expect_ident()
        } else if self.at(TokenKind::Super) && self.peek(1).kind == TokenKind::Dot {
            self.advance(); // super
            self.advance(); // .
            self.expect_ident()
        } else if let Some(function_named) = leading
            .is_none()
            .then(|| self.function_named_formal())
            .flatten()
        {
            function_named
        } else {
            self.expect_ident()
        };
        if matches!(kind, ParamKind::Named { .. })
            && initializing_this
            && name.text(self).starts_with('_')
            && !self.version.supports(Feature::PrivateNamedParameters)
        {
            self.error_at(
                name.span().start,
                "This requires the 'private-named-parameters' language feature",
            );
        }

        // Generic old-style function-typed formal: `name<T>(params)`. Dart allows
        // type parameters on a function-typed formal parameter (unlike inside a
        // generic function TYPE, which the caller keeps rejecting).
        self.parse_type_params();
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

        if self.at_any(&[TokenKind::Eq, TokenKind::Colon]) {
            self.advance();
            self.parse_expr();
        }

        marker.complete(self, NodeKind::FormalParam)
    }

    /// An old-style function-typed formal whose *name* is the built-in
    /// identifier `Function`: `void map(void Function({int? e}))`. The input
    /// reads as an unnamed function type, so when a full type parse ends in a
    /// `Function(...)` suffix that no parameter name follows, re-read that
    /// suffix as the parameter's name and parameter list, keeping the cursor
    /// before `Function`. Returns the `Function` name.
    fn function_named_formal(&mut self) -> Option<CompletedMarker> {
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
            if ty.kind() != NodeKind::FunctionType {
                return None;
            }
            if !ends_param {
                return None;
            }
            parser.restore_cursor(keyword);
            let marker = parser.start();
            parser.advance(); // Function
            Some(marker.complete(parser, NodeKind::Identifier))
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn function_named_formal_keeps_first_pass_errors() {
        let parsed = crate::parse("void f(void Function(int,,)) {}");
        assert!(
            parsed
                .errors
                .iter()
                .any(|error| error.message == "expected type, got Comma")
        );
        assert!(
            parsed
                .errors
                .iter()
                .any(|error| error.message == "expected identifier, got Comma")
        );
    }
}
