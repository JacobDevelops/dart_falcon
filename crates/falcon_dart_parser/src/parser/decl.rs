use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

use super::{ParseError, Parser};
use crate::lexer::{Lexer, filter_trivia};

impl<'src> Parser<'src> {
    // ── Directives ────────────────────────────────────────────────────────────

    pub(super) fn try_parse_library_directive(&mut self) -> Option<LibraryDirective> {
        // May be preceded by metadata: `@Deprecated('x') library foo;`.
        if self.peek_kind_after_annotations() != TokenKind::Library {
            return None;
        }
        let start = self.cur().offset;
        let annotations = self.parse_annotations();
        self.advance(); // library
        // Dart 3 allows bare `library;` with no name.
        let mut name = Vec::new();
        if self.is_ident_like() {
            name.push(self.expect_ident());
            while self.eat(TokenKind::Dot).is_some() {
                name.push(self.expect_ident());
            }
        }
        self.expect(TokenKind::Semicolon);
        Some(LibraryDirective {
            annotations,
            name,
            span: self.span_from(start),
        })
    }

    pub(super) fn try_parse_part_of(&mut self) -> Option<PartOfDirective> {
        // `part of …`, optionally preceded by metadata: `@A part of foo;`.
        let i = self.index_after_annotations(self.pos);
        let part = self.tokens.get(i);
        let of = self.tokens.get(i + 1);
        let is_part_of = part.is_some_and(|t| t.kind == TokenKind::Part)
            && of.is_some_and(|t| t.kind == TokenKind::Ident && self.tok_text(t) == "of");
        if !is_part_of {
            return None;
        }
        let start = self.cur().offset;
        let annotations = self.parse_annotations();
        self.advance(); // part
        self.advance(); // of
        let (uri, name) = if matches!(self.cur().kind, TokenKind::StringLit) {
            (Some(self.parse_uri()), vec![])
        } else {
            let mut segs = vec![self.expect_ident()];
            while self.eat(TokenKind::Dot).is_some() {
                segs.push(self.expect_ident());
            }
            (None, segs)
        };
        self.expect(TokenKind::Semicolon);
        Some(PartOfDirective {
            annotations,
            uri,
            name,
            span: self.span_from(start),
        })
    }

    pub(super) fn try_parse_part(&mut self) -> Option<PartDirective> {
        let start = self.cur().offset;
        let annotations = self.parse_annotations();
        self.expect(TokenKind::Part);
        if !matches!(self.cur().kind, TokenKind::StringLit) {
            self.error("expected string literal in part directive");
            return None;
        }
        let uri = self.parse_uri();
        self.expect(TokenKind::Semicolon);
        Some(PartDirective {
            annotations,
            uri,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_import(&mut self) -> ImportDirective {
        let start = self.cur().offset;
        let annotations = self.parse_annotations();
        self.expect(TokenKind::Import);
        let uri = self.parse_uri();
        let configurable_uris = self.parse_configurable_uris();
        let is_deferred = self.eat(TokenKind::Deferred).is_some();
        let as_name = if self.eat(TokenKind::As).is_some() {
            Some(self.expect_ident())
        } else {
            None
        };
        let combinators = self.parse_import_combinators();
        self.expect(TokenKind::Semicolon);
        ImportDirective {
            annotations,
            uri,
            configurable_uris,
            is_deferred,
            as_name,
            combinators,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_export(&mut self) -> ExportDirective {
        let start = self.cur().offset;
        let annotations = self.parse_annotations();
        self.expect(TokenKind::Export);
        let uri = self.parse_uri();
        let configurable_uris = self.parse_configurable_uris();
        let combinators = self.parse_import_combinators();
        self.expect(TokenKind::Semicolon);
        ExportDirective {
            annotations,
            uri,
            configurable_uris,
            combinators,
            span: self.span_from(start),
        }
    }

    /// Parse zero or more configurable-URI clauses:
    /// `if (dotted.name [== 'value']) 'uri'`.
    fn parse_configurable_uris(&mut self) -> Vec<ConfigurableUri> {
        let mut uris = Vec::new();
        while self.at(TokenKind::If) {
            let start = self.cur().offset;
            self.advance(); // if
            self.expect(TokenKind::LParen);
            let mut test = vec![self.expect_ident()];
            while self.eat(TokenKind::Dot).is_some() {
                test.push(self.expect_ident());
            }
            let value = if self.eat(TokenKind::EqEq).is_some() {
                Some(self.parse_string_lit())
            } else {
                None
            };
            self.expect(TokenKind::RParen);
            let uri = self.parse_uri();
            uris.push(ConfigurableUri {
                test,
                value,
                uri,
                span: self.span_from(start),
            });
        }
        uris
    }

    fn parse_import_combinators(&mut self) -> Vec<ImportCombinator> {
        let mut combinators = Vec::new();
        loop {
            if self.at(TokenKind::Show) {
                let start = self.cur().offset;
                self.advance();
                let names = self.parse_ident_list();
                combinators.push(ImportCombinator::Show(names, self.span_from(start)));
            } else if self.at(TokenKind::Hide) {
                let start = self.cur().offset;
                self.advance();
                let names = self.parse_ident_list();
                combinators.push(ImportCombinator::Hide(names, self.span_from(start)));
            } else {
                break;
            }
        }
        combinators
    }

    fn parse_ident_list(&mut self) -> Vec<Identifier> {
        let mut ids = vec![self.expect_ident()];
        while self.eat(TokenKind::Comma).is_some() {
            if self.is_ident_like() {
                ids.push(self.expect_ident());
            }
        }
        ids
    }

    // ── Top-level declarations ────────────────────────────────────────────────

    pub(super) fn parse_top_level_decl(&mut self) -> Option<TopLevelDecl> {
        let annotations = self.parse_annotations();
        let start = self.cur().offset;

        // Collect class-level modifiers
        let mut is_abstract = false;
        let mut is_interface = false;
        let mut is_base = false;
        let mut is_final = false;
        let mut is_sealed = false;

        loop {
            match self.cur().kind {
                TokenKind::Abstract => {
                    is_abstract = true;
                    self.advance();
                }
                TokenKind::Interface => {
                    is_interface = true;
                    self.advance();
                }
                TokenKind::Base => {
                    is_base = true;
                    self.advance();
                }
                TokenKind::Final => {
                    is_final = true;
                    self.advance();
                }
                TokenKind::Sealed => {
                    is_sealed = true;
                    self.advance();
                }
                // `static` is only legal on class members. At top level it is an
                // extraneous modifier — unless it is actually the name of a
                // top-level function (`static() => ...`, `static<T>() ...`), where
                // a `(`/`<` follows. Report and recover by skipping it.
                TokenKind::Static
                    if !matches!(self.peek(1).kind, TokenKind::LParen | TokenKind::Lt) =>
                {
                    self.error("Can't have modifier 'static' here".to_string());
                    self.advance();
                }
                _ => break,
            }
        }

        let modifiers = ClassModifiers {
            is_abstract,
            is_interface,
            is_base,
            is_final,
            is_sealed,
        };

        match self.cur().kind {
            TokenKind::Class => Some(self.parse_class_or_type_alias(annotations, modifiers, start)),
            TokenKind::Mixin => {
                // `mixin class` → MixinClass
                if self.peek(1).kind == TokenKind::Class {
                    self.advance(); // mixin
                    Some(TopLevelDecl::MixinClass(self.parse_mixin_class(
                        annotations,
                        is_abstract,
                        is_base,
                        start,
                    )))
                } else {
                    Some(TopLevelDecl::Mixin(self.parse_mixin(
                        annotations,
                        is_base,
                        start,
                    )))
                }
            }
            TokenKind::Enum => Some(TopLevelDecl::Enum(self.parse_enum(annotations, start))),
            TokenKind::Extension => {
                // `extension type` or plain `extension`
                if self.peek(1).kind == TokenKind::Type {
                    Some(TopLevelDecl::ExtensionType(
                        self.parse_extension_type(annotations, start),
                    ))
                } else {
                    Some(TopLevelDecl::Extension(
                        self.parse_extension(annotations, start),
                    ))
                }
            }
            TokenKind::Typedef => Some(TopLevelDecl::TypeAlias(
                self.parse_typedef(annotations, start),
            )),
            _ => {
                // Could be top-level function or variable
                self.try_parse_top_level_func_or_var(annotations, start, is_final)
            }
        }
    }

    // ── Class ─────────────────────────────────────────────────────────────────

    fn parse_class_or_type_alias(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: ClassModifiers,
        start: usize,
    ) -> TopLevelDecl {
        self.expect(TokenKind::Class);
        let name = self.expect_ident();
        let type_params = self.parse_type_params();

        // Mixin-application class: `class MA = Super with M [implements I];`
        if self.eat(TokenKind::Eq).is_some() {
            let superclass = self.parse_type();
            // A mixin-application class requires a `with` clause; without it this is
            // not a valid class type alias.
            self.expect(TokenKind::With);
            let with_clause = self.parse_type_list();
            let implements = if self.eat(TokenKind::Implements).is_some() {
                self.parse_type_list()
            } else {
                Vec::new()
            };
            self.expect(TokenKind::Semicolon);
            return TopLevelDecl::ClassTypeAlias(ClassTypeAliasDecl {
                annotations,
                modifiers,
                name,
                type_params,
                superclass,
                with_clause,
                implements,
                span: self.span_from(start),
            });
        }

        let extends = if self.eat(TokenKind::Extends).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let with_clause = if self.eat(TokenKind::With).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        let implements = if self.eat(TokenKind::Implements).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LBrace);
        let members = self.parse_class_body_in(Some(name.name.clone()));
        self.expect(TokenKind::RBrace);
        TopLevelDecl::Class(ClassDecl {
            annotations,
            modifiers,
            name,
            type_params,
            extends,
            with_clause,
            implements,
            members,
            span: self.span_from(start),
        })
    }

    fn parse_mixin(
        &mut self,
        annotations: Vec<Annotation>,
        is_base: bool,
        start: usize,
    ) -> MixinDecl {
        self.expect(TokenKind::Mixin);
        let name = self.expect_ident();
        let type_params = self.parse_type_params();
        let on_clause = if self.eat(TokenKind::On).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        let implements = if self.eat(TokenKind::Implements).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LBrace);
        let members = self.parse_class_body_in(None);
        self.expect(TokenKind::RBrace);
        MixinDecl {
            annotations,
            is_base,
            name,
            type_params,
            on_clause,
            implements,
            members,
            span: self.span_from(start),
        }
    }

    fn parse_mixin_class(
        &mut self,
        annotations: Vec<Annotation>,
        is_abstract: bool,
        is_base: bool,
        start: usize,
    ) -> MixinClassDecl {
        self.expect(TokenKind::Class);
        let name = self.expect_ident();
        let type_params = self.parse_type_params();
        let extends = if self.eat(TokenKind::Extends).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let with_clause = if self.eat(TokenKind::With).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        let implements = if self.eat(TokenKind::Implements).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LBrace);
        let members = self.parse_class_body_in(Some(name.name.clone()));
        self.expect(TokenKind::RBrace);
        MixinClassDecl {
            annotations,
            is_abstract,
            is_base,
            name,
            type_params,
            extends,
            with_clause,
            implements,
            members,
            span: self.span_from(start),
        }
    }

    fn parse_type_list(&mut self) -> Vec<DartType> {
        let mut types = vec![self.parse_type()];
        while self.eat(TokenKind::Comma).is_some() {
            if self.is_type_start_at_cur() {
                types.push(self.parse_type());
            }
        }
        types
    }

    fn is_type_start_at_cur(&self) -> bool {
        self.is_ident_like()
            || matches!(
                self.cur().kind,
                TokenKind::Void | TokenKind::Dynamic | TokenKind::LParen
            )
    }

    /// With a `(` at `self.peek(offset)`, scan to its matching `)` and decide
    /// whether that parenthesized group is a record *type* prefixing a member —
    /// i.e. its `)` (with an optional trailing `?`) is followed by a member name,
    /// a `Function` return-type suffix, or a `get`/`set`/`operator` keyword. This
    /// distinguishes `static (int, int) m()` (record return type) from a member
    /// named after the keyword such as `external()`.
    fn record_type_prefixes_member(&self, offset: usize) -> bool {
        use TokenKind::*;
        let mut i = self.pos + offset;
        if self.tokens.get(i).map(|t| &t.kind) != Some(&LParen) {
            return false;
        }
        let mut depth = 0usize;
        let cap = i + 512;
        while let Some(tok) = self.tokens.get(i) {
            if i > cap {
                return false;
            }
            match tok.kind {
                LParen => depth += 1,
                RParen => {
                    depth -= 1;
                    if depth == 0 {
                        let mut j = i + 1;
                        if self.tokens.get(j).map(|t| &t.kind) == Some(&Qmark) {
                            j += 1;
                        }
                        return match self.tokens.get(j).map(|t| &t.kind) {
                            Some(Function | Get | Set | Operator) => true,
                            // `async`/`sync` after `)` is ambiguous: it may be a
                            // function *body* modifier (`external() async {}`,
                            // `covariant() sync* {}` — the `(...)` is a parameter
                            // list, the keyword the member's own name) OR the member
                            // *name* itself prefixed by a record return type
                            // (`static (int,int) async()`, `late (int,int) async;`).
                            // The token *after* async/sync decides: a name is
                            // followed by `(`/`<`/`;`/`=`/`,`; a body modifier by
                            // `{`/`=>`/`*` (generator marker).
                            Some(Async | Sync) => matches!(
                                self.tokens.get(j + 1).map(|t| &t.kind),
                                Some(LParen | Lt | Semicolon | Eq | Comma)
                            ),
                            Some(k) => Self::kind_is_ident_like(k),
                            None => false,
                        };
                    }
                }
                Eof | Semicolon => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    // ── Class body ────────────────────────────────────────────────────────────

    fn parse_class_body(&mut self) -> Vec<ClassMember> {
        let mut members = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            members.push(self.parse_class_member());
        }
        members
    }

    /// Parse a class body with the enclosing-constructor-name context set (and
    /// restored afterwards). `ctor_name` is `Some(type name)` when the enclosing
    /// declaration may declare constructors, or `None` for mixins/extensions
    /// whose untyped members are always methods.
    fn parse_class_body_in(&mut self, ctor_name: Option<String>) -> Vec<ClassMember> {
        let saved = self.enclosing_ctor_name.take();
        self.enclosing_ctor_name = ctor_name;
        let members = self.parse_class_body();
        self.enclosing_ctor_name = saved;
        members
    }

    /// True when the enclosing type admits constructors (class, mixin class,
    /// enum, extension type) — i.e. an untyped `name(...)` member can be one.
    fn enclosing_allows_ctor(&self) -> bool {
        self.enclosing_ctor_name.is_some()
    }

    /// True when `text` names the enclosing type, so an untyped `text(...)`
    /// member is that type's unnamed constructor rather than a method.
    fn name_is_enclosing_type(&self, text: &str) -> bool {
        self.enclosing_ctor_name.as_deref() == Some(text)
    }

    fn parse_class_member(&mut self) -> ClassMember {
        let start = self.cur().offset;
        let annotations = self.parse_annotations();
        let mut is_static = false;
        let mut is_abstract = false;
        let mut is_external = false;
        let mut is_covariant = false;
        let mut is_late = false;
        let mut is_final = false;
        let mut is_const = false;
        let mut is_override = false;
        let mut is_async = false;

        loop {
            // A built-in-identifier modifier (`static`, `abstract`, `external`,
            // `covariant`, `late`) directly followed by `=`/`;`/`,`/`(` is the
            // member's own name, not a modifier — e.g. the field named `static` in
            // `static const static = 1;`, or a method `external()`. A real modifier
            // is always followed by another modifier, a type, or the member name.
            // The `(` case is refined: a following record *type* (`static (int, int)
            // m()`, `late (int, int) r;`) keeps the keyword as a modifier.
            if matches!(
                self.cur().kind,
                TokenKind::Static
                    | TokenKind::Abstract
                    | TokenKind::External
                    | TokenKind::Covariant
                    | TokenKind::Late
            ) {
                let keyword_is_name = match self.peek(1).kind {
                    TokenKind::Eq | TokenKind::Semicolon | TokenKind::Comma => true,
                    TokenKind::LParen => !self.record_type_prefixes_member(1),
                    _ => false,
                };
                if keyword_is_name {
                    break;
                }
            }
            match self.cur().kind {
                TokenKind::Static => {
                    is_static = true;
                    self.advance();
                }
                TokenKind::Abstract => {
                    is_abstract = true;
                    self.advance();
                }
                TokenKind::External => {
                    is_external = true;
                    self.advance();
                }
                TokenKind::Covariant => {
                    is_covariant = true;
                    self.advance();
                }
                TokenKind::Late => {
                    is_late = true;
                    self.advance();
                }
                TokenKind::Final => {
                    is_final = true;
                    self.advance();
                }
                TokenKind::Const => {
                    is_const = true;
                    self.advance();
                }
                // `async` is a body suffix, not a leading modifier; only treat it
                // as one when it is not the member's own name (`async() {}`,
                // `async<T>() {}`) or a field named `async`.
                TokenKind::Async
                    if !matches!(
                        self.peek(1).kind,
                        TokenKind::LParen | TokenKind::Lt | TokenKind::Semicolon | TokenKind::Eq
                    ) =>
                {
                    is_async = true;
                    self.advance();
                }
                // @override is handled via annotations, but 'override' as a built-in
                // identifier can lead a member. When it is immediately followed by
                // `=`/`;`/`,`/`(`/`<` it is the member's own name (`const override = 1;`,
                // `override() {}`), not a modifier.
                _ if self.is_ident_like()
                    && self.cur_text() == "override"
                    && !matches!(
                        self.peek(1).kind,
                        TokenKind::Eq
                            | TokenKind::Semicolon
                            | TokenKind::Comma
                            | TokenKind::LParen
                            | TokenKind::Lt
                    ) =>
                {
                    is_override = true;
                    self.advance();
                }
                _ => break,
            }
        }
        let _ = is_override;

        // Getter
        if self.at(TokenKind::Get) && self.is_ident_like_at_offset(1) {
            return ClassMember::Getter(self.parse_getter(
                annotations,
                is_static,
                is_abstract,
                is_external,
                None,
                start,
            ));
        }

        // Setter
        if self.at(TokenKind::Set) && self.is_ident_like_at_offset(1) {
            return ClassMember::Setter(self.parse_setter(
                annotations,
                is_static,
                is_abstract,
                is_external,
                start,
            ));
        }

        // Operator overload
        if self.at_operator_decl() {
            return ClassMember::Operator(self.parse_operator(
                annotations,
                is_external,
                None,
                start,
            ));
        }

        // Factory constructor. `factory` is a built-in identifier, so shipped code
        // also uses it as a plain member name (`late final factory = 1;`, a method
        // `factory() {}`, a getter `factory<T>`). A real factory keyword is always
        // followed by the enclosing type name; when it is followed by
        // `=`/`;`/`,`/`(`/`<` instead it is the member's own name.
        if self.at(TokenKind::Factory)
            && !matches!(
                self.peek(1).kind,
                TokenKind::Eq
                    | TokenKind::Semicolon
                    | TokenKind::Comma
                    | TokenKind::LParen
                    | TokenKind::Lt
            )
        {
            return ClassMember::Constructor(self.parse_factory_constructor(
                annotations,
                is_external,
                start,
            ));
        }

        // `var` keyword — untyped mutable field
        if self.eat(TokenKind::Var).is_some() {
            return ClassMember::Field(self.parse_field_tail(
                annotations,
                is_static,
                is_abstract,
                is_external,
                is_covariant,
                is_late,
                is_final,
                is_const,
                None,
                start,
            ));
        }

        // Try to parse as field or method
        self.parse_field_or_method(
            annotations,
            is_static,
            is_abstract,
            is_external,
            is_covariant,
            is_late,
            is_final,
            is_const,
            is_async,
            start,
        )
    }

    fn is_ident_like_at_offset(&self, offset: usize) -> bool {
        Self::kind_is_ident_like(&self.peek(offset).kind)
    }

    /// True when the `operator` keyword at the cursor actually declares an
    /// operator overload — i.e. it is followed by an overloadable operator symbol
    /// (or `[` for `[]`/`[]=`). `operator` is a built-in identifier, so shipped
    /// code also uses it as a plain member name (`Token? operator;`); there the
    /// next token is `;`/`=`/`,`/`(`, not an operator, so this returns false.
    fn at_operator_decl(&self) -> bool {
        use TokenKind::*;
        self.at(Operator)
            && matches!(
                self.peek(1).kind,
                LBracket
                    | Lt
                    | Gt
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

    // `X.new(...)` declares the default constructor. `new` is a keyword token, so
    // it is not `is_ident_like`; it is only a name in constructor-name position.
    fn is_ctor_name_at_offset(&self, offset: usize) -> bool {
        self.is_ident_like_at_offset(offset) || self.peek(offset).kind == TokenKind::New
    }

    pub(super) fn expect_ctor_name(&mut self) -> Identifier {
        if self.at(TokenKind::New) {
            let tok = self.advance();
            return Identifier::new(self.tok_text(&tok).to_string(), Self::tok_span(&tok));
        }
        self.expect_ident()
    }

    fn parse_getter(
        &mut self,
        annotations: Vec<Annotation>,
        is_static: bool,
        is_abstract: bool,
        is_external: bool,
        return_type: Option<DartType>,
        start: usize,
    ) -> GetterDecl {
        self.advance(); // get
        let name = self.expect_ident();
        // A getter body may carry an `async` marker (`get x async { ... }`).
        let (is_async, _is_generator) = self.parse_async_marker();
        let body = self.parse_function_body();
        GetterDecl {
            annotations,
            is_static,
            is_abstract,
            is_external,
            is_async,
            return_type,
            name,
            body,
            span: self.span_from(start),
        }
    }

    fn parse_setter(
        &mut self,
        annotations: Vec<Annotation>,
        is_static: bool,
        is_abstract: bool,
        is_external: bool,
        start: usize,
    ) -> SetterDecl {
        self.advance(); // set
        let name = self.expect_ident();
        self.expect(TokenKind::LParen);
        // A setter's single value parameter accepts the same leading modifiers as
        // any formal (`set x(covariant final T v)`) and a trailing comma
        // (`set x(int a,)`), which method parameter lists already allow.
        let _ = self.eat(TokenKind::Covariant);
        let _ = self.eat(TokenKind::Final);
        let _ = self.eat(TokenKind::Var);
        let param_type = if self.is_type_start_at_cur() && self.peek(1).kind != TokenKind::RParen {
            let saved = self.pos;
            let ty = self.parse_type();
            if self.is_ident_like() {
                Some(ty)
            } else {
                self.rewind_to(saved);
                None
            }
        } else {
            None
        };
        let param = self.expect_ident();
        self.eat(TokenKind::Comma); // optional trailing comma
        self.expect(TokenKind::RParen);
        let body = self.parse_function_body();
        SetterDecl {
            annotations,
            is_static,
            is_abstract,
            is_external,
            is_async: false,
            param_type,
            name,
            param,
            body,
            span: self.span_from(start),
        }
    }

    fn parse_operator(
        &mut self,
        annotations: Vec<Annotation>,
        is_external: bool,
        return_type: Option<DartType>,
        start: usize,
    ) -> OperatorDecl {
        self.advance(); // operator
        // `[]` and `[]=` are lexed as separate bracket tokens; all other operators
        // are a single token.
        let op = if self.at(TokenKind::LBracket) {
            self.advance(); // [
            self.expect(TokenKind::RBracket); // ]
            if self.eat(TokenKind::Eq).is_some() {
                "[]=".to_string()
            } else {
                "[]".to_string()
            }
        } else {
            let op = self.cur_text().to_string();
            self.advance();
            op
        };
        let params = self.parse_formal_param_list();
        let body = self.parse_function_body();
        OperatorDecl {
            annotations,
            is_external,
            return_type,
            op,
            params,
            body,
            span: self.span_from(start),
        }
    }

    fn parse_factory_constructor(
        &mut self,
        annotations: Vec<Annotation>,
        is_external: bool,
        start: usize,
    ) -> ConstructorDecl {
        self.advance(); // factory
        let name = self.expect_ident();
        let constructor_name = if self.eat(TokenKind::Dot).is_some() {
            Some(self.expect_ctor_name())
        } else {
            None
        };
        let params = self.parse_formal_param_list();
        // Redirecting factory: `factory C() = D;` / `= D.named;` / `= D<int>;`.
        let redirect = if self.at(TokenKind::Eq) {
            Some(self.parse_redirected_constructor())
        } else {
            None
        };
        let body = if redirect.is_some() {
            // A redirecting factory has no body but must be terminated: `= D;`.
            self.expect(TokenKind::Semicolon);
            None
        } else {
            self.parse_function_body()
        };
        ConstructorDecl {
            annotations,
            is_const: false,
            is_factory: true,
            is_external,
            name,
            constructor_name,
            params,
            initializers: Vec::new(),
            redirect,
            body,
            span: self.span_from(start),
        }
    }

    /// Parse a redirecting-factory target after `=`: `Type[.name]`.
    fn parse_redirected_constructor(&mut self) -> RedirectedConstructor {
        let start = self.cur().offset;
        self.expect(TokenKind::Eq);
        let type_ = self.parse_type();
        let constructor_name = if self.eat(TokenKind::Dot).is_some() {
            Some(self.expect_ctor_name())
        } else {
            None
        };
        RedirectedConstructor {
            type_,
            constructor_name,
            span: self.span_from(start),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_field_or_method(
        &mut self,
        annotations: Vec<Annotation>,
        is_static: bool,
        is_abstract: bool,
        is_external: bool,
        is_covariant: bool,
        is_late: bool,
        is_final: bool,
        is_const: bool,
        outer_is_async: bool,
        start: usize,
    ) -> ClassMember {
        let saved = self.pos;

        if is_final || is_const || is_late {
            // Const constructor: `const ClassName(` or `const ClassName.name(`.
            // Only when the enclosing type admits constructors and the name is a
            // dotted (named) form or matches the enclosing type — otherwise this
            // is a const field and falls through below.
            if is_const && self.is_ident_like() && self.enclosing_allows_ctor() {
                let name_text = self.cur_text().to_string();
                let p1 = self.peek(1).kind.clone();
                let dotted = p1 == TokenKind::Dot && self.is_ctor_name_at_offset(2);
                // Both the dotted (`C.named(`) and parenthesized (`C(`) forms name a
                // constructor only when the leading name is the enclosing type.
                let is_ctor =
                    self.name_is_enclosing_type(&name_text) && (dotted || p1 == TokenKind::LParen);
                if is_ctor {
                    let ctor_saved = self.pos;
                    let name_tok = self.cur().clone();
                    self.advance();
                    let name = Identifier::new(
                        self.tok_text(&name_tok).to_string(),
                        Self::tok_span(&name_tok),
                    );
                    let constructor_name = if self.eat(TokenKind::Dot).is_some() {
                        Some(self.expect_ctor_name())
                    } else {
                        None
                    };
                    if self.at(TokenKind::LParen) {
                        let params = self.parse_formal_param_list();
                        let initializers = self.parse_constructor_initializers();
                        let body = self.parse_function_body();
                        return ClassMember::Constructor(ConstructorDecl {
                            annotations,
                            is_const: true,
                            is_factory: false,
                            is_external,
                            name,
                            constructor_name,
                            params,
                            initializers,
                            redirect: None,
                            body,
                            span: self.span_from(start),
                        });
                    }
                    // Not a constructor (e.g. `const qualified.Type field = ...`) — rollback
                    self.rewind_to(ctor_saved);
                }
            }
            let field_type = if self.is_type_start_at_cur() {
                let saved2 = self.pos;
                let ty = self.parse_type();
                if self.is_ident_like() {
                    Some(ty)
                } else {
                    self.rewind_to(saved2);
                    None
                }
            } else {
                None
            };
            return ClassMember::Field(self.parse_field_tail(
                annotations,
                is_static,
                false,
                is_external,
                is_covariant,
                is_late,
                is_final,
                is_const,
                field_type,
                start,
            ));
        }

        // Speculative: parse type, see what follows
        if self.is_type_start_at_cur() {
            let ty = self.parse_type();

            // Typed getter: `ReturnType get name ...`
            if self.at(TokenKind::Get) && self.is_ident_like_at_offset(1) {
                return ClassMember::Getter(self.parse_getter(
                    annotations,
                    is_static,
                    is_abstract,
                    is_external,
                    Some(ty),
                    start,
                ));
            }
            // Typed setter: `ReturnType set name ...`
            if self.at(TokenKind::Set) && self.is_ident_like_at_offset(1) {
                return ClassMember::Setter(self.parse_setter(
                    annotations,
                    is_static,
                    is_abstract,
                    is_external,
                    start,
                ));
            }
            // Typed operator: `ReturnType operator ...`
            if self.at_operator_decl() {
                return ClassMember::Operator(self.parse_operator(
                    annotations,
                    is_external,
                    Some(ty),
                    start,
                ));
            }

            if self.is_ident_like() {
                let name_tok = self.cur().clone();
                self.advance();
                // Method
                if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                    let name = Identifier::new(
                        self.tok_text(&name_tok).to_string(),
                        Self::tok_span(&name_tok),
                    );
                    let type_params = self.parse_type_params();
                    let params = self.parse_formal_param_list();
                    let (async_from_marker, is_generator) = self.parse_async_marker();
                    let is_method_async = outer_is_async || async_from_marker;
                    let body = self.parse_function_body();
                    let is_method_abstract = is_abstract || body.is_none();
                    return ClassMember::Method(MethodDecl {
                        annotations,
                        is_static,
                        is_abstract: is_method_abstract,
                        is_external,
                        is_async: is_method_async,
                        is_generator,
                        return_type: Some(ty),
                        name,
                        type_params,
                        params,
                        body,
                        span: self.span_from(start),
                    });
                }
                // Field
                let field_name = Identifier::new(
                    self.tok_text(&name_tok).to_string(),
                    Self::tok_span(&name_tok),
                );
                let init = if self.eat(TokenKind::Eq).is_some() {
                    Some(self.parse_expr())
                } else {
                    None
                };
                let mut declarators = vec![VarDeclarator {
                    name: field_name,
                    initializer: init,
                    span: self.span_from(start),
                }];
                while self.eat(TokenKind::Comma).is_some() {
                    let n = self.expect_ident();
                    let i = if self.eat(TokenKind::Eq).is_some() {
                        Some(self.parse_expr())
                    } else {
                        None
                    };
                    let sp = self.span_from(start);
                    declarators.push(VarDeclarator {
                        name: n,
                        initializer: i,
                        span: sp,
                    });
                }
                self.expect(TokenKind::Semicolon);
                return ClassMember::Field(FieldDecl {
                    annotations,
                    is_static,
                    is_abstract,
                    is_external,
                    is_covariant,
                    is_late,
                    is_final,
                    is_const,
                    field_type: Some(ty),
                    declarators,
                    span: self.span_from(start),
                });
            }
            self.rewind_to(saved);
        }

        // No type prefix — `name(...)`, `name.ctor(...)`, or `name<T>(...)`.
        // With no return type this is a constructor only when the enclosing type
        // admits constructors AND the name is a dotted (named) form or equals the
        // enclosing type name; type parameters or an `async`/`sync*` marker (which
        // a constructor may not have) also force a method. Everything else — e.g.
        // `foo() {}` in `class C` — is an untyped method with no return type.
        if self.is_ident_like() {
            let name_tok = self.cur().clone();
            let name_text = self.tok_text(&name_tok).to_string();
            self.advance();
            let has_ctor_dot = self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1);
            if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) || has_ctor_dot {
                let name = Identifier::new(name_text.clone(), Self::tok_span(&name_tok));
                let constructor_name = if self.eat(TokenKind::Dot).is_some() {
                    Some(self.expect_ctor_name())
                } else {
                    None
                };
                let type_params = self.parse_type_params();
                if self.at(TokenKind::LParen) {
                    let params = self.parse_formal_param_list();
                    let has_type_params = !type_params.is_empty();
                    let has_marker = matches!(self.cur().kind, TokenKind::Async | TokenKind::Sync);
                    // A dotted (`C.named(`) or parenthesized (`C(`) form names a
                    // constructor only when the leading name is the enclosing type —
                    // `D.named()` in `class C` is not a constructor of C.
                    let is_constructor = self.enclosing_allows_ctor()
                        && !has_type_params
                        && !has_marker
                        && self.name_is_enclosing_type(&name_text);
                    if !is_constructor {
                        let (async_from_marker, is_generator) = self.parse_async_marker();
                        let body = self.parse_function_body();
                        let is_method_abstract = is_abstract || body.is_none();
                        return ClassMember::Method(MethodDecl {
                            annotations,
                            is_static,
                            is_abstract: is_method_abstract,
                            is_external,
                            is_async: outer_is_async || async_from_marker,
                            is_generator,
                            return_type: None,
                            name,
                            type_params,
                            params,
                            body,
                            span: self.span_from(start),
                        });
                    }
                    let initializers = self.parse_constructor_initializers();
                    let body = self.parse_function_body();
                    return ClassMember::Constructor(ConstructorDecl {
                        annotations,
                        is_const,
                        is_factory: false,
                        is_external,
                        name,
                        constructor_name,
                        params,
                        initializers,
                        redirect: None,
                        body,
                        span: self.span_from(start),
                    });
                }
            }
            self.rewind_to(saved);
        }

        // Give up — emit error node and skip to next member
        let span = self.cur_span();
        self.error("could not parse class member");
        self.synchronize(&[TokenKind::Semicolon, TokenKind::RBrace]);
        self.eat(TokenKind::Semicolon);
        ClassMember::Error(ErrorNode {
            message: "could not parse class member".into(),
            span,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_field_tail(
        &mut self,
        annotations: Vec<Annotation>,
        is_static: bool,
        is_abstract: bool,
        is_external: bool,
        is_covariant: bool,
        is_late: bool,
        is_final: bool,
        is_const: bool,
        field_type: Option<DartType>,
        start: usize,
    ) -> FieldDecl {
        let name = self.expect_ident();
        let init = if self.eat(TokenKind::Eq).is_some() {
            Some(self.parse_expr())
        } else {
            None
        };
        let mut declarators = vec![VarDeclarator {
            name,
            initializer: init,
            span: self.span_from(start),
        }];
        while self.eat(TokenKind::Comma).is_some() {
            let n = self.expect_ident();
            let i = if self.eat(TokenKind::Eq).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            let sp = self.span_from(start);
            declarators.push(VarDeclarator {
                name: n,
                initializer: i,
                span: sp,
            });
        }
        self.expect(TokenKind::Semicolon);
        FieldDecl {
            annotations,
            is_static,
            is_abstract,
            is_external,
            is_covariant,
            is_late,
            is_final,
            is_const,
            field_type,
            declarators,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_async_marker(&mut self) -> (bool, bool) {
        match self.cur().kind {
            TokenKind::Async => {
                self.advance();
                let is_gen = self.eat(TokenKind::Star).is_some();
                (true, is_gen)
            }
            TokenKind::Sync => {
                self.advance();
                let is_gen = self.eat(TokenKind::Star).is_some();
                (false, is_gen)
            }
            _ => (false, false),
        }
    }

    fn parse_constructor_initializers(&mut self) -> Vec<ConstructorInitializer> {
        if self.eat(TokenKind::Colon).is_none() {
            return Vec::new();
        }
        let mut inits = Vec::new();
        loop {
            let start = self.cur().offset;
            let init = match self.cur().kind {
                TokenKind::Super => {
                    self.advance();
                    let call_name = if self.eat(TokenKind::Dot).is_some() {
                        Some(self.expect_ident())
                    } else {
                        None
                    };
                    let args = self.parse_arg_list();
                    ConstructorInitializer::SuperCall {
                        call_name,
                        args,
                        span: self.span_from(start),
                    }
                }
                TokenKind::This => {
                    self.advance();
                    let call_name = if self.eat(TokenKind::Dot).is_some() {
                        Some(self.expect_ident())
                    } else {
                        None
                    };
                    if self.at(TokenKind::LParen) {
                        let args = self.parse_arg_list();
                        ConstructorInitializer::ThisCall {
                            call_name,
                            args,
                            span: self.span_from(start),
                        }
                    } else {
                        // this.field = value
                        let field = call_name
                            .unwrap_or_else(|| Identifier::new("<error>", self.cur_span()));
                        self.expect(TokenKind::Eq);
                        let value = self.parse_ctor_init_value();
                        ConstructorInitializer::FieldInit {
                            field,
                            value,
                            span: self.span_from(start),
                        }
                    }
                }
                TokenKind::Assert => {
                    self.advance();
                    self.expect(TokenKind::LParen);
                    let condition = self.parse_expr();
                    let message =
                        if self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
                            let m = self.parse_expr();
                            self.eat(TokenKind::Comma); // optional trailing comma
                            Some(m)
                        } else {
                            None
                        };
                    self.expect(TokenKind::RParen);
                    ConstructorInitializer::Assert {
                        condition,
                        message,
                        span: self.span_from(start),
                    }
                }
                _ => {
                    // field = value
                    let field = self.expect_ident();
                    self.expect(TokenKind::Eq);
                    let value = self.parse_ctor_init_value();
                    ConstructorInitializer::FieldInit {
                        field,
                        value,
                        span: self.span_from(start),
                    }
                }
            };
            inits.push(init);
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        inits
    }

    /// Parse a constructor field-initializer value with [`Parser::in_ctor_init_value`]
    /// set, so a trailing `(...) {}` is disambiguated as a parenthesized expression
    /// followed by the constructor body rather than a lambda.
    fn parse_ctor_init_value(&mut self) -> Expr {
        let prev = self.in_ctor_init_value;
        self.in_ctor_init_value = true;
        let value = self.parse_expr();
        self.in_ctor_init_value = prev;
        value
    }

    // ── Enum ──────────────────────────────────────────────────────────────────

    fn parse_enum(&mut self, annotations: Vec<Annotation>, start: usize) -> EnumDecl {
        self.expect(TokenKind::Enum);
        let name = self.expect_ident();
        let type_params = self.parse_type_params();
        let with_clause = if self.eat(TokenKind::With).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        let implements = if self.eat(TokenKind::Implements).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LBrace);

        let mut variants = Vec::new();
        let mut members = Vec::new();

        // Enums admit constructors, so an untyped member named after the enum is
        // one; every other untyped member is a method.
        let saved_ctor_name = self.enclosing_ctor_name.take();
        self.enclosing_ctor_name = Some(name.name.clone());

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let v_start = self.cur().offset;
            let v_annotations = self.parse_annotations();
            if !self.is_ident_like() {
                break;
            }
            let v_name = self.expect_ident();
            let type_args = if self.at(TokenKind::Lt) {
                self.parse_type_args()
            } else {
                Vec::new()
            };
            // Enhanced-enum constant invoking a named (or `.new`) constructor:
            // `a.foo(1)`, `a.new(1)`.
            let constructor_name = if self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1) {
                self.advance(); // .
                Some(self.expect_ctor_name())
            } else {
                None
            };
            let args = if self.at(TokenKind::LParen) {
                Some(self.parse_arg_list())
            } else {
                None
            };
            variants.push(EnumVariant {
                annotations: v_annotations,
                name: v_name,
                type_args,
                constructor_name,
                args,
                span: self.span_from(v_start),
            });
            if self.eat(TokenKind::Comma).is_none() {
                // No comma — check for `;` separator before class members
                if self.eat(TokenKind::Semicolon).is_some() {
                    while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                        members.push(self.parse_class_member());
                    }
                }
                break;
            }
            // After comma, may have ; and then members
            if self.eat(TokenKind::Semicolon).is_some() {
                while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                    members.push(self.parse_class_member());
                }
                break;
            }
        }
        self.enclosing_ctor_name = saved_ctor_name;
        self.expect(TokenKind::RBrace);
        EnumDecl {
            annotations,
            name,
            type_params,
            with_clause,
            implements,
            variants,
            members,
            span: self.span_from(start),
        }
    }

    // ── Extension ─────────────────────────────────────────────────────────────

    fn parse_extension(&mut self, annotations: Vec<Annotation>, start: usize) -> ExtensionDecl {
        self.expect(TokenKind::Extension);
        let name = if self.is_ident_like() && !self.at(TokenKind::On) {
            Some(self.expect_ident())
        } else {
            None
        };
        let type_params = self.parse_type_params();
        self.eat(TokenKind::On);
        let on_type = self.parse_type();
        self.expect(TokenKind::LBrace);
        let members = self.parse_class_body_in(None);
        self.expect(TokenKind::RBrace);
        ExtensionDecl {
            annotations,
            name,
            type_params,
            on_type,
            members,
            span: self.span_from(start),
        }
    }

    fn parse_extension_type(
        &mut self,
        annotations: Vec<Annotation>,
        start: usize,
    ) -> ExtensionTypeDecl {
        self.expect(TokenKind::Extension);
        self.expect(TokenKind::Type);
        let is_const = self.eat(TokenKind::Const).is_some();
        let name = self.expect_ident();
        let type_params = self.parse_type_params();
        // Optional named representation constructor: `extension type Name._(int it)`.
        let rep_start = self.cur().offset;
        let constructor_name = if self.eat(TokenKind::Dot).is_some() {
            Some(self.expect_ctor_name())
        } else {
            None
        };
        // representation: (Type fieldName)
        self.expect(TokenKind::LParen);
        let field_type = self.parse_type();
        let field_name = self.expect_ident();
        self.expect(TokenKind::RParen);
        let representation = ExtensionTypeRepresentation {
            constructor_name,
            field_type,
            field_name,
            span: self.span_from(rep_start),
        };
        let implements = if self.eat(TokenKind::Implements).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LBrace);
        let members = self.parse_class_body_in(Some(name.name.clone()));
        self.expect(TokenKind::RBrace);
        ExtensionTypeDecl {
            annotations,
            is_const,
            name,
            type_params,
            representation,
            implements,
            members,
            span: self.span_from(start),
        }
    }

    // ── Typedef ───────────────────────────────────────────────────────────────

    fn parse_typedef(&mut self, annotations: Vec<Annotation>, start: usize) -> TypeAliasDecl {
        self.expect(TokenKind::Typedef);
        let saved = self.pos;

        // Modern form: `typedef Name<T> = Type;`
        if self.is_ident_like() {
            let name = self.expect_ident();
            let type_params = self.parse_type_params();
            if self.eat(TokenKind::Eq).is_some() {
                let aliased = self.parse_type();
                self.expect(TokenKind::Semicolon);
                return TypeAliasDecl {
                    annotations,
                    name,
                    type_params,
                    aliased,
                    span: self.span_from(start),
                };
            }
            self.rewind_to(saved);
        }

        // Legacy form: `typedef [returnType] Name<T>(params);` — synthesize a
        // function type from the return type and parameter list.
        let leading = self.parse_type();
        let ty_start = leading.span().start;
        let (return_type, name, type_params) = if self.is_ident_like() {
            // `leading` was the return type; the name follows.
            let name = self.expect_ident();
            let type_params = self.parse_type_params();
            (Some(leading), name, type_params)
        } else {
            // No return type; `leading` is the name (with type args as type params).
            let (name, type_params) = Self::named_type_to_alias_head(leading);
            (None, name, type_params)
        };
        let params = self.parse_formal_param_list();
        self.expect(TokenKind::Semicolon);
        let fn_span = Span::new(ty_start, self.cur().offset);
        let aliased = DartType::Function(Box::new(FunctionType {
            return_type: return_type.map(Box::new),
            type_params: Vec::new(),
            params: Self::formals_to_function_type_params(&params),
            is_nullable: false,
            span: fn_span,
        }));
        TypeAliasDecl {
            annotations,
            name,
            type_params,
            aliased,
            span: self.span_from(start),
        }
    }

    /// Split a parsed named type into an alias head: its name and any type
    /// arguments reinterpreted as type parameters (`Comparator<T>` → `Comparator`,
    /// `[T]`). Used for legacy typedefs that lack a return type.
    fn named_type_to_alias_head(ty: DartType) -> (Identifier, Vec<TypeParam>) {
        match ty {
            DartType::Named(n) => {
                let span = n.span.clone();
                let name = n
                    .segments
                    .into_iter()
                    .next_back()
                    .unwrap_or_else(|| Identifier::new("<error>", span.clone()));
                let type_params = n
                    .type_args
                    .into_iter()
                    .map(|arg| {
                        let sp = arg.span().clone();
                        let tp_name = match arg {
                            DartType::Named(mut a) => a
                                .segments
                                .pop()
                                .unwrap_or_else(|| Identifier::new("<error>", sp.clone())),
                            other => Identifier::new("<error>", other.span().clone()),
                        };
                        TypeParam {
                            annotations: Vec::new(),
                            name: tp_name,
                            bound: None,
                            span: sp,
                        }
                    })
                    .collect();
                (name, type_params)
            }
            other => (Identifier::new("<error>", other.span().clone()), Vec::new()),
        }
    }

    /// Convert a formal parameter list into function-type parameters for a
    /// synthesized function type (legacy typedefs, etc.).
    fn formals_to_function_type_params(params: &FormalParamList) -> Vec<FunctionTypeParam> {
        let mut out = Vec::new();
        let dynamic_type = |sp: Span| DartType::Dynamic { span: sp };
        for p in &params.positional {
            out.push(FunctionTypeParam {
                name: Some(p.name.clone()),
                param_type: p
                    .param_type
                    .clone()
                    .unwrap_or_else(|| dynamic_type(p.span.clone())),
                is_required: true,
                is_named: false,
            });
        }
        for p in &params.optional_positional {
            out.push(FunctionTypeParam {
                name: Some(p.name.clone()),
                param_type: p
                    .param_type
                    .clone()
                    .unwrap_or_else(|| dynamic_type(p.span.clone())),
                is_required: false,
                is_named: false,
            });
        }
        for p in &params.named {
            out.push(FunctionTypeParam {
                name: Some(p.name.clone()),
                param_type: p
                    .param_type
                    .clone()
                    .unwrap_or_else(|| dynamic_type(p.span.clone())),
                is_required: p.is_required,
                is_named: true,
            });
        }
        out
    }

    // ── Top-level function / variable ─────────────────────────────────────────

    fn try_parse_top_level_func_or_var(
        &mut self,
        annotations: Vec<Annotation>,
        start: usize,
        outer_is_final: bool,
    ) -> Option<TopLevelDecl> {
        let is_external = self.eat(TokenKind::External).is_some();

        // Modifiers for variable. Dart orders these `late` before `final`/`const`
        // (`late final int x`), so consume `late` first. `final` may already have
        // been consumed by the outer modifier loop.
        let is_late = self.eat(TokenKind::Late).is_some();
        let is_final = outer_is_final || self.eat(TokenKind::Final).is_some();
        let is_const = self.eat(TokenKind::Const).is_some();
        let _ = self.eat(TokenKind::Var);

        // Return type (optional). A bare `get`/`set` directly followed by the
        // accessor name is an *untyped* top-level getter/setter (`get foo => 1;`),
        // not a return type — leave the keyword for the getter/setter handling
        // below, which the speculative type parse would otherwise consume as a
        // type named `get`/`set`.
        let at_untyped_accessor =
            self.at_any(&[TokenKind::Get, TokenKind::Set]) && self.is_ident_like_at_offset(1);
        let return_type = if !at_untyped_accessor && self.is_type_start_at_cur() {
            let saved = self.pos;
            let ty = self.parse_type();
            // Accept the type only if followed by an ident-like token (the name).
            // Special case: if followed by `get`/`set` + ident, that is a getter/setter.
            if self.at(TokenKind::Get) || self.at(TokenKind::Set) || self.is_ident_like() {
                Some(ty)
            } else {
                self.rewind_to(saved);
                None
            }
        } else {
            None
        };

        // Top-level getter: `(ReturnType) get name { ... }`
        if self.at(TokenKind::Get) && self.is_ident_like_at_offset(1) {
            self.advance(); // get
            let name = self.expect_ident();
            let (is_async, is_generator) = self.parse_async_marker();
            let body = self.parse_function_body();
            let empty_params = FormalParamList {
                positional: vec![],
                optional_positional: vec![],
                named: vec![],
                span: Span::default(),
            };
            return Some(TopLevelDecl::Function(FunctionDecl {
                annotations,
                is_external,
                is_async,
                is_generator,
                is_getter: true,
                is_setter: false,
                return_type,
                name,
                type_params: vec![],
                params: empty_params,
                body,
                span: self.span_from(start),
            }));
        }

        // Top-level setter: `(ReturnType) set name(param) { ... }`
        if self.at(TokenKind::Set) && self.is_ident_like_at_offset(1) {
            self.advance(); // set
            let name = self.expect_ident();
            let params = self.parse_formal_param_list();
            let (is_async, is_generator) = self.parse_async_marker();
            let body = self.parse_function_body();
            return Some(TopLevelDecl::Function(FunctionDecl {
                annotations,
                is_external,
                is_async,
                is_generator,
                is_getter: false,
                is_setter: true,
                return_type,
                name,
                type_params: vec![],
                params,
                body,
                span: self.span_from(start),
            }));
        }

        // Name
        if !self.is_ident_like() {
            // Nothing recognisable
            return None;
        }
        let name = self.expect_ident();

        // Function declaration
        if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
            let type_params = self.parse_type_params();
            let params = self.parse_formal_param_list();
            let (is_async, is_generator) = self.parse_async_marker();
            let body = self.parse_function_body();
            return Some(TopLevelDecl::Function(FunctionDecl {
                annotations,
                is_external,
                is_async,
                is_generator,
                is_getter: false,
                is_setter: false,
                return_type,
                name,
                type_params,
                params,
                body,
                span: self.span_from(start),
            }));
        }

        // Variable declaration
        let init = if self.eat(TokenKind::Eq).is_some() {
            Some(self.parse_expr())
        } else {
            None
        };
        let mut declarators = vec![VarDeclarator {
            name,
            initializer: init,
            span: self.span_from(start),
        }];
        while self.eat(TokenKind::Comma).is_some() {
            let n = self.expect_ident();
            let i = if self.eat(TokenKind::Eq).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            let sp = self.span_from(start);
            declarators.push(VarDeclarator {
                name: n,
                initializer: i,
                span: sp,
            });
        }
        self.eat(TokenKind::Semicolon);
        Some(TopLevelDecl::Variable(TopLevelVarDecl {
            annotations,
            is_external,
            is_final,
            is_const,
            is_late,
            var_type: return_type,
            declarators,
            span: self.span_from(start),
        }))
    }

    // ── String literal helper ─────────────────────────────────────────────────

    /// Parse a directive URI, concatenating adjacent string literals as Dart does
    /// for any string context: `import 'package:foo' '/bar.dart';`.
    fn parse_uri(&mut self) -> StringLitNode {
        let mut node = self.parse_string_lit();
        while self.at(TokenKind::StringLit) {
            let next = self.parse_string_lit();
            let mut interpolations = node.interpolations;
            interpolations.extend(next.interpolations);
            node = StringLitNode {
                raw: node.raw + &next.raw,
                value: node.value + &next.value,
                span: node.span.merge(&next.span),
                interpolations,
            };
        }
        node
    }

    pub(super) fn parse_string_lit(&mut self) -> StringLitNode {
        let tok = self.cur().clone();
        let raw = self.tok_text(&tok).to_string();
        let span = Self::tok_span(&tok);
        let is_string_lit = matches!(tok.kind, TokenKind::StringLit);
        if is_string_lit {
            self.advance();
        } else {
            self.error("expected string literal");
        }
        // Simple value extraction: strip outer quotes
        let value = strip_quotes(&raw);
        let interpolations = if is_string_lit {
            let (interps, interp_errors) = scan_interpolations(self.src, tok.offset, &raw);
            // Surface any interpolation sub-parse diagnostics to the enclosing
            // program (their offsets are already absolute).
            self.errors.extend(interp_errors);
            interps
        } else {
            Vec::new()
        };
        StringLitNode {
            raw,
            value,
            span,
            interpolations,
        }
    }
}

// ── String interpolation scanning ─────────────────────────────────────────────

/// Scan a single string literal's raw text for interpolation regions and parse
/// each into an expression with an absolute source span.
///
/// `lit_start` is the absolute byte offset of `raw` within `src`. Raw strings
/// (`r'...'`) yield no interpolations. A `${expr}` whose inner slice fails to
/// parse cleanly records no interpolation, but its parse diagnostics (with
/// absolute source offsets) are returned alongside so the enclosing program can
/// surface them.
fn scan_interpolations(
    src: &str,
    lit_start: usize,
    raw: &str,
) -> (Vec<StringInterpolation>, Vec<ParseError>) {
    let bytes = raw.as_bytes();
    if bytes.first() == Some(&b'r') {
        return (Vec::new(), Vec::new());
    }
    let dlen = if raw.starts_with("'''") || raw.starts_with("\"\"\"") {
        3
    } else if raw.starts_with('\'') || raw.starts_with('"') {
        1
    } else {
        return (Vec::new(), Vec::new());
    };
    let content_start = dlen;
    // A well-formed literal ends with the same delimiter; if it does not (e.g. a
    // merged-adjacent fragment) scan to the end of the text instead.
    let content_end = raw.len().saturating_sub(dlen).max(content_start);

    let mut out = Vec::new();
    let mut errs = Vec::new();
    let mut i = content_start;
    while i < content_end {
        match bytes[i] {
            b'\\' => i += 2,
            b'$' if i + 1 < content_end && bytes[i + 1] == b'{' => {
                if let Some(close) = find_interp_close(bytes, i + 1, content_end) {
                    let inner_start = i + 2;
                    let (interp, interp_errs) =
                        parse_interp_expr(src, lit_start + inner_start, lit_start + close);
                    if let Some(interp) = interp {
                        out.push(interp);
                    }
                    errs.extend(interp_errs);
                    i = close + 1;
                } else {
                    i += 1;
                }
            }
            b'$' if i + 1 < content_end && is_interp_ident_start(bytes[i + 1]) => {
                let id_start = i + 1;
                let mut k = id_start + 1;
                while k < content_end && is_interp_ident_continue(bytes[k]) {
                    k += 1;
                }
                let span = Span::new(lit_start + id_start, lit_start + k);
                out.push(StringInterpolation {
                    expr: Expr::Ident(Identifier::new(raw[id_start..k].to_string(), span.clone())),
                    span,
                });
                i = k;
            }
            _ => i += 1,
        }
    }
    (out, errs)
}

/// Find the byte index of the `}` closing a `${...}` region that opens at
/// `open_brace` (the `{`). Tracks brace depth, skips escapes, steps over nested
/// Dart strings, and skips line/block comments so braces inside strings or
/// comments do not throw off the count. Returns `None` when unbalanced within
/// `end`.
fn find_interp_close(bytes: &[u8], open_brace: usize, end: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut i = open_brace + 1;
    while i < end {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            // A `//` line comment runs to the end of the line; its braces must not
            // affect `depth`.
            b'/' if i + 1 < end && bytes[i + 1] == b'/' => {
                i += 2;
                while i < end && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            // A `/* ... */` block comment (Dart nests them); its braces must not
            // affect `depth`. Unbalanced within `end` is treated as unbalanced.
            b'/' if i + 1 < end && bytes[i + 1] == b'*' => {
                i += 2;
                let mut cdepth = 1usize;
                while i < end && cdepth > 0 {
                    if bytes[i] == b'/' && i + 1 < end && bytes[i + 1] == b'*' {
                        cdepth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && i + 1 < end && bytes[i + 1] == b'/' {
                        cdepth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if cdepth != 0 {
                    return None;
                }
                continue;
            }
            b'\'' | b'"' => {
                i = skip_nested_string(bytes, i, end)?;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Given `open` at a quote byte, return the index just past the matching closing
/// quote (handling triple quotes and escapes). `None` when unterminated.
fn skip_nested_string(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
    let quote = bytes[open];
    // A raw-string `r` prefix suppresses escapes: `r'\'` closes at its second
    // quote, so a backslash must not consume the next byte here.
    let raw = open > 0 && bytes[open - 1] == b'r';
    let triple = open + 2 < end && bytes[open + 1] == quote && bytes[open + 2] == quote;
    let mut i = if triple { open + 3 } else { open + 1 };
    while i < end {
        let b = bytes[i];
        if b == b'\\' && !raw {
            i += 2;
            continue;
        }
        if b == quote {
            if triple {
                if i + 2 < end && bytes[i + 1] == quote && bytes[i + 2] == quote {
                    return Some(i + 3);
                }
                i += 1;
            } else {
                return Some(i + 1);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Lex and parse the source slice `src[start..end]` as a standalone expression,
/// shifting token offsets so the resulting AST carries absolute spans. The first
/// element is `Some` only when the slice parses to a single non-error expression
/// consuming the whole slice; otherwise it is `None` and the second element
/// carries the sub-parse diagnostics (with absolute offsets) so the caller can
/// surface them instead of dropping the failure silently.
fn parse_interp_expr(
    src: &str,
    start: usize,
    end: usize,
) -> (Option<StringInterpolation>, Vec<ParseError>) {
    if start >= end {
        return (None, Vec::new());
    }
    let slice = &src[start..end];
    let mut tokens = Lexer::new(slice).tokenize();
    for tok in &mut tokens {
        tok.offset += start;
    }
    let tokens = filter_trivia(tokens);
    let mut sub = Parser::new(tokens, src);
    let expr = sub.parse_expr();
    if !sub.errors.is_empty() || !sub.at(TokenKind::Eof) || matches!(expr, Expr::Error { .. }) {
        return (None, sub.errors);
    }
    let span = expr.span().clone();
    (Some(StringInterpolation { expr, span }), Vec::new())
}

// `$` is excluded so `$a$b` reads as two interpolations, and `.`/`(` end a
// simple `$identifier` region (mirroring Dart's interpolation grammar).
fn is_interp_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_interp_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

// Minimal quote-stripping for string literal values (not full escape handling).
fn strip_quotes(raw: &str) -> String {
    let raw = raw.strip_prefix('r').unwrap_or(raw);
    let (triple, q) = if raw.starts_with("\"\"\"") || raw.starts_with("'''") {
        (true, &raw[..3])
    } else if raw.starts_with('"') || raw.starts_with('\'') {
        (false, &raw[..1])
    } else {
        return raw.to_string();
    };
    let inner_start = if triple { 3 } else { 1 };
    let inner_end = if raw.ends_with(q) {
        raw.len() - q.len()
    } else {
        raw.len()
    };
    raw[inner_start..inner_end.max(inner_start)].to_string()
}
