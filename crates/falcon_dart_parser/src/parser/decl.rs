use falcon_syntax::ast::*;
use falcon_syntax::token::{SyntaxStore, Token, TokenKind};
use falcon_syntax::{Feature, LanguageVersion};

use super::{
    ParseError, Parser,
    lexical::{InterpolationRegion, visit_interpolations},
};

#[derive(Clone, Copy, Default)]
struct MemberModifiers {
    is_static: bool,
    is_abstract: bool,
    is_external: bool,
    is_covariant: bool,
    is_late: bool,
    is_final: bool,
    is_const: bool,
    is_async: bool,
}

impl MemberModifiers {
    fn keyword(self) -> VarKeyword {
        Parser::var_keyword(self.is_const, self.is_final)
    }
}

impl<'src> Parser<'src> {
    pub(super) fn try_parse_library_directive(&mut self) -> Option<LibraryDirective> {
        // May be preceded by metadata: `@Deprecated('x') library foo;`.
        if self.peek_kind_after_annotations() != TokenKind::Library {
            return None;
        }
        let start = self.cur().offset();
        let annotations = self.parse_annotations();
        self.advance(); // library
        // Dart 3 allows bare `library;` with no name.
        let mut name = Vec::new();
        if self.cur().kind.is_identifier_like() {
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
        let start = self.cur().offset();
        let annotations = self.parse_annotations();
        self.advance(); // part
        self.advance(); // of
        let (uri, name) = if matches!(self.cur().kind, TokenKind::StringLit) {
            (Some(self.parse_adjacent_strings()), vec![])
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
        let start = self.cur().offset();
        let annotations = self.parse_annotations();
        self.expect(TokenKind::Part);
        if !matches!(self.cur().kind, TokenKind::StringLit) {
            self.error("expected string literal in part directive");
            return None;
        }
        let uri = self.parse_adjacent_strings();
        self.expect(TokenKind::Semicolon);
        Some(PartDirective {
            annotations,
            uri,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_import(&mut self) -> ImportDirective {
        let start = self.cur().offset();
        let annotations = self.parse_annotations();
        self.expect(TokenKind::Import);
        let uri = self.parse_adjacent_strings();
        let configurable_uris = self.parse_configurable_uris();
        let is_deferred = self.eat(TokenKind::Deferred).is_some();
        let as_name = self.eat(TokenKind::As).map(|_| self.expect_ident());
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
        let start = self.cur().offset();
        let annotations = self.parse_annotations();
        self.expect(TokenKind::Export);
        let uri = self.parse_adjacent_strings();
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
            let start = self.cur().offset();
            self.advance(); // if
            self.expect(TokenKind::LParen);
            let mut test = vec![self.expect_ident()];
            while self.eat(TokenKind::Dot).is_some() {
                test.push(self.expect_ident());
            }
            let value = self.eat(TokenKind::EqEq).map(|_| self.parse_string_lit());
            self.expect(TokenKind::RParen);
            let uri = self.parse_adjacent_strings();
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
                let start = self.cur().offset();
                self.advance();
                let names = self.parse_ident_list();
                combinators.push(ImportCombinator::Show(names, self.span_from(start)));
            } else if self.at(TokenKind::Hide) {
                let start = self.cur().offset();
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
            if self.cur().kind.is_identifier_like() {
                ids.push(self.expect_ident());
            }
        }
        ids
    }

    pub(super) fn parse_top_level_decl(&mut self) -> Option<TopLevelDecl> {
        let annotations = self.parse_annotations();
        let start = self.cur().offset();

        let mut modifiers = ClassModifiers::default();

        loop {
            match self.cur().kind {
                TokenKind::Abstract => {
                    modifiers.is_abstract = true;
                    self.advance();
                }
                TokenKind::Interface => {
                    modifiers.restriction = Some(ClassRestriction::Interface);
                    self.advance();
                }
                TokenKind::Base => {
                    modifiers.restriction = Some(ClassRestriction::Base);
                    self.advance();
                }
                TokenKind::Final => {
                    modifiers.restriction = Some(ClassRestriction::Final);
                    self.advance();
                }
                TokenKind::Sealed => {
                    modifiers.restriction = Some(ClassRestriction::Sealed);
                    self.advance();
                }
                // `static` is only legal on class members. At top level it is an
                // extraneous modifier — unless it is actually the name of a
                // top-level function (`static() => ...`, `static<T>() ...`), where
                // a `(`/`<` follows. Report and recover by skipping it.
                TokenKind::Static
                    if !matches!(self.peek(1).kind, TokenKind::LParen | TokenKind::Lt) =>
                {
                    self.error("Can't have modifier 'static' here");
                    self.advance();
                }
                _ => break,
            }
        }

        let is_base = modifiers.restriction == Some(ClassRestriction::Base);
        let is_final = modifiers.restriction == Some(ClassRestriction::Final);

        match self.cur().kind {
            TokenKind::Class => Some(self.parse_class_or_type_alias(annotations, modifiers, start)),
            TokenKind::Mixin => {
                // `mixin class` → MixinClass
                if self.peek(1).kind == TokenKind::Class {
                    self.advance(); // mixin
                    Some(TopLevelDecl::MixinClass(self.parse_mixin_class(
                        annotations,
                        modifiers.is_abstract,
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

    fn parse_class_or_type_alias(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: ClassModifiers,
        start: usize,
    ) -> TopLevelDecl {
        self.expect(TokenKind::Class);
        let (name, type_params, primary_constructor) = self.parse_name_maybe_primary(false);

        // Mixin-application class: `class MA = Super with M [implements I];`
        if primary_constructor.is_none() && self.eat(TokenKind::Eq).is_some() {
            let superclass = self.parse_type();
            // A mixin-application class requires a `with` clause; without it this is
            // not a valid class type alias.
            self.expect(TokenKind::With);
            let with_clause = self.parse_type_list();
            let implements = self.parse_type_clause(TokenKind::Implements);
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

        let extends = self.eat(TokenKind::Extends).map(|_| self.parse_type());
        let with_clause = self.parse_type_clause(TokenKind::With);
        let implements = self.parse_type_clause(TokenKind::Implements);
        let members = self.parse_class_body_in(Some(&self.src[name.span.start..name.span.end]));
        TopLevelDecl::Class(ClassDecl {
            annotations,
            modifiers,
            name,
            type_params,
            primary_constructor,
            extends,
            with_clause,
            implements,
            members,
            span: self.span_from(start),
        })
    }

    /// `<classNameMaybePrimary>`: `[const] Name<T>`, optionally followed by a
    /// Dart 3.13 primary constructor `[.name](params)`.
    fn parse_name_maybe_primary(
        &mut self,
        representation: bool,
    ) -> (Identifier, Vec<TypeParam>, Option<PrimaryConstructor>) {
        let start = self.cur().offset();
        let is_const = self.eat(TokenKind::Const).is_some();
        let name = self.expect_ident();
        let type_params = self.parse_type_params();
        let named = self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1);
        if !(is_const || named || self.at(TokenKind::LParen)) {
            return (name, type_params, None);
        }
        if !representation && !self.version.supports(Feature::PrimaryConstructors) {
            self.error("This requires the 'primary-constructors' language feature");
        }
        let constructor_name = named.then(|| {
            self.advance(); // .
            self.expect_ctor_name()
        });
        let params = self.parse_formal_param_list();
        let span = Span::new(start, params.span.end);
        let primary_constructor = PrimaryConstructor {
            is_const,
            name_span: name.span,
            constructor_name,
            params,
            span,
        };
        (name, type_params, Some(primary_constructor))
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
        let on_clause = self.parse_type_clause(TokenKind::On);
        let implements = self.parse_type_clause(TokenKind::Implements);
        let members = self.parse_class_body_in(None);
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
        let (name, type_params, primary_constructor) = self.parse_name_maybe_primary(false);
        let extends = self.eat(TokenKind::Extends).map(|_| self.parse_type());
        let with_clause = self.parse_type_clause(TokenKind::With);
        let implements = self.parse_type_clause(TokenKind::Implements);
        let members = self.parse_class_body_in(Some(&self.src[name.span.start..name.span.end]));
        MixinClassDecl {
            annotations,
            is_abstract,
            is_base,
            name,
            type_params,
            primary_constructor,
            extends,
            with_clause,
            implements,
            members,
            span: self.span_from(start),
        }
    }

    fn parse_type_clause(&mut self, keyword: TokenKind) -> Vec<DartType> {
        if self.eat(keyword).is_some() {
            self.parse_type_list()
        } else {
            Vec::new()
        }
    }

    fn parse_type_list(&mut self) -> Vec<DartType> {
        let mut types = vec![self.parse_type()];
        while self.eat(TokenKind::Comma).is_some() {
            if self.is_type_start() {
                types.push(self.parse_type());
            }
        }
        types
    }

    /// With a `(` at `self.peek(offset)`, scan to its matching `)` and decide
    /// whether that parenthesized group is a record *type* prefixing a member —
    /// i.e. its `)` (with an optional trailing `?`) is followed by a member name,
    /// a `Function` return-type suffix, or a `get`/`set`/`operator` keyword. This
    /// distinguishes `static (int, int) m()` (record return type) from a member
    /// named after the keyword such as `external()`.
    fn record_type_prefixes_member(&self, offset: usize) -> bool {
        use TokenKind::*;
        let i = self.pos + offset;
        if self.kind_at(i) != LParen {
            return false;
        }
        let Some(close) = self.matching_close(
            i,
            super::expr::Brackets::ParenOnly,
            |kind| matches!(kind, Eof | Semicolon),
            Some(513),
        ) else {
            return false;
        };
        let mut j = close + 1;
        if self.kind_at(j) == Qmark {
            j += 1;
        }
        match self.kind_at(j) {
            Function | Get | Set | Operator => true,
            // A name follows the record type; a body marker follows parameters.
            Async | Sync => matches!(self.kind_at(j + 1), LParen | Lt | Semicolon | Eq | Comma),
            kind => kind.is_identifier_like(),
        }
    }

    fn parse_class_body(&mut self) -> Vec<ClassMember> {
        let mut members = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            members.push(self.parse_class_member());
        }
        members
    }

    /// Parse a braced class body with the enclosing-constructor-name context
    /// set (and restored afterwards). `ctor_name` is `Some(type name)` when the
    /// enclosing declaration may declare constructors, or `None` for
    /// mixins/extensions whose untyped members are always methods. Dart 3.13
    /// also accepts `;` for an empty body (`abstract class Marker;`).
    fn parse_class_body_in(&mut self, ctor_name: Option<&'src str>) -> Vec<ClassMember> {
        if let Some(semicolon) = self.eat(TokenKind::Semicolon) {
            if !self.version.supports(Feature::PrimaryConstructors) {
                self.error_at(
                    semicolon.offset(),
                    "This requires the 'primary-constructors' language feature",
                );
            }
            return Vec::new();
        }
        self.expect(TokenKind::LBrace);
        let members = self.with_ctx(|ctx| ctx.ctor_name = ctor_name, Self::parse_class_body);
        self.expect(TokenKind::RBrace);
        members
    }

    fn parse_member_modifiers(&mut self) -> MemberModifiers {
        let mut modifiers = MemberModifiers::default();

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
                    modifiers.is_static = true;
                    self.advance();
                }
                TokenKind::Abstract => {
                    modifiers.is_abstract = true;
                    self.advance();
                }
                TokenKind::External => {
                    modifiers.is_external = true;
                    self.advance();
                }
                TokenKind::Covariant => {
                    modifiers.is_covariant = true;
                    self.advance();
                }
                TokenKind::Late => {
                    modifiers.is_late = true;
                    self.advance();
                }
                TokenKind::Final => {
                    modifiers.is_final = true;
                    self.advance();
                }
                TokenKind::Const => {
                    modifiers.is_const = true;
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
                    modifiers.is_async = true;
                    self.advance();
                }
                // @override is handled via annotations, but 'override' as a built-in
                // identifier can lead a member. When it is immediately followed by
                // `=`/`;`/`,`/`(`/`<` it is the member's own name (`const override = 1;`,
                // `override() {}`), not a modifier.
                _ if self.cur().kind.is_identifier_like()
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
                    self.advance();
                }
                _ => break,
            }
        }
        modifiers
    }

    fn parse_class_member(&mut self) -> ClassMember {
        let start = self.cur().offset();
        let annotations = self.parse_annotations();
        let modifiers = self.parse_member_modifiers();

        if self.at(TokenKind::Get) && self.peek(1).kind.is_identifier_like() {
            return ClassMember::Getter(self.parse_getter(annotations, modifiers, None, start));
        }

        if self.at(TokenKind::Set) && self.peek(1).kind.is_identifier_like() {
            return ClassMember::Setter(self.parse_setter(annotations, modifiers, start));
        }

        if self.at_operator_decl() {
            return ClassMember::Operator(self.parse_operator(annotations, modifiers, None, start));
        }

        // Body part of a primary constructor: `this : x = 1 { ... }`, `this;`.
        if self.at(TokenKind::This)
            && self.ctx.ctor_name.is_some()
            && matches!(
                self.peek(1).kind,
                TokenKind::Colon | TokenKind::LBrace | TokenKind::Semicolon | TokenKind::Arrow
            )
        {
            if !self.version.supports(Feature::PrimaryConstructors) {
                self.error("This requires the 'primary-constructors' language feature");
            }
            let this_token = self.advance();
            let this_span = Span::new(this_token.offset(), this_token.offset() + this_token.len());
            let initializers = self.parse_constructor_initializers();
            let body = self.parse_function_body(AsyncMarker::Sync);
            return ClassMember::PrimaryConstructorBody(PrimaryConstructorBody {
                annotations,
                this_span,
                initializers,
                body,
                span: self.span_from(start),
            });
        }

        // Dart 3.13 generative constructor syntax: `new(...)`, `new name(...)`,
        // with the usual `const`/`external` modifiers.
        if self.at(TokenKind::New) && self.ctx.ctor_name.is_some() {
            if !self.version.supports(Feature::PrimaryConstructors) {
                self.error("This requires the 'primary-constructors' language feature");
            }
            return ClassMember::Constructor(self.parse_new_constructor(
                annotations,
                modifiers,
                start,
            ));
        }

        // Factory constructor. `factory` is a built-in identifier, so shipped code
        // also uses it as a plain member name (`late final factory = 1;`, a getter
        // `factory<T>`). A real factory keyword is followed by the enclosing type
        // name, by a constructor name (`factory name(`), or — in a type that
        // declares constructors, from Dart 3.13 (the `factory()` head of the
        // primary-constructors feature) — directly by its parameters
        // (`factory(`); when it is followed by `=`/`;`/`,`/`<` (or by `(`
        // otherwise) it is the member's own name.
        if self.at(TokenKind::Factory)
            && match self.peek(1).kind {
                TokenKind::Eq | TokenKind::Semicolon | TokenKind::Comma | TokenKind::Lt => false,
                TokenKind::LParen => {
                    self.ctx.ctor_name.is_some()
                        && self.version.supports(Feature::PrimaryConstructors)
                }
                _ => true,
            }
        {
            return ClassMember::Constructor(self.parse_factory_constructor(
                annotations,
                modifiers,
                start,
            ));
        }

        // `var` keyword — untyped mutable field
        if self.eat(TokenKind::Var).is_some() {
            let name = self.expect_ident();
            return ClassMember::Field(self.finish_field(
                annotations,
                modifiers,
                None,
                name,
                start,
            ));
        }

        self.parse_field_or_method(annotations, modifiers, start)
    }

    /// True when the `operator` keyword at the cursor actually declares an
    /// operator overload — i.e. it is followed by an overloadable operator symbol
    /// (or `[` for `[]`/`[]=`). `operator` is a built-in identifier, so shipped
    /// code also uses it as a plain member name (`Token? operator;`); there the
    /// next token is `;`/`=`/`,`/`(`, not an operator, so this returns false.
    fn at_operator_decl(&self) -> bool {
        self.at(TokenKind::Operator)
            && (self.peek(1).kind == TokenKind::LBracket
                || self.peek(1).kind.is_user_definable_operator())
    }

    // `X.new(...)` declares the default constructor. `new` is a keyword token, so
    // it is accepted only in constructor-name position.
    fn is_ctor_name_at_offset(&self, offset: usize) -> bool {
        self.peek(offset).kind.is_identifier_like() || self.peek(offset).kind == TokenKind::New
    }

    pub(super) fn expect_ctor_name(&mut self) -> Identifier {
        if self.at(TokenKind::New) {
            let tok = self.advance();
            return self.tok_ident(&tok);
        }
        self.expect_ident()
    }

    fn parse_getter(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        return_type: Option<DartType>,
        start: usize,
    ) -> GetterDecl {
        let MemberModifiers {
            is_static,
            is_abstract,
            is_external,
            ..
        } = modifiers;
        self.advance(); // get
        let name = self.expect_ident();
        // A getter body may carry an `async` marker (`get x async { ... }`).
        let async_marker = self.parse_async_marker();
        let body = self.parse_function_body(async_marker);
        GetterDecl {
            annotations,
            is_static,
            // Like methods, a body-less non-external getter is abstract.
            is_abstract: is_abstract || (body.is_none() && !is_external),
            is_external,
            async_marker,
            return_type,
            name,
            body,
            span: self.span_from(start),
        }
    }

    fn parse_setter(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        start: usize,
    ) -> SetterDecl {
        let MemberModifiers {
            is_static,
            is_abstract,
            is_external,
            ..
        } = modifiers;
        self.advance(); // set
        let name = self.expect_ident();
        self.expect(TokenKind::LParen);
        // A setter's single value parameter accepts the same leading modifiers as
        // any formal (`set x(covariant final T v)`) and a trailing comma
        // (`set x(int a,)`), which method parameter lists already allow.
        let param_annotations = self.parse_annotations();
        let _ = self.eat(TokenKind::Covariant);
        let _ = self.eat(TokenKind::Final);
        let _ = self.eat(TokenKind::Var);
        let mut param_type = if self.peek(1).kind != TokenKind::RParen {
            self.try_parse_type_followed_by(TokenKind::is_identifier_like)
        } else {
            None
        };
        let param = self.expect_ident();
        let type_params = self.parse_type_params();
        if self.at(TokenKind::LParen) {
            let params = self.parse_formal_param_list();
            let is_nullable = self.eat(TokenKind::Qmark).is_some();
            let type_start = param_type
                .as_ref()
                .map_or(param.span.start, |ty| ty.span().start);
            param_type = Some(DartType::Function(Box::new(FunctionType {
                return_type: param_type.map(Box::new),
                type_params,
                params: Self::formals_to_function_type_params(&params),
                is_nullable,
                span: self.span_from(type_start),
            })));
        }
        self.eat(TokenKind::Comma); // optional trailing comma
        self.expect(TokenKind::RParen);
        let body = self.parse_function_body(AsyncMarker::Sync);
        SetterDecl {
            annotations,
            is_static,
            is_abstract: is_abstract || (body.is_none() && !is_external),
            is_external,
            param_type,
            param_annotations,
            name,
            param,
            body,
            span: self.span_from(start),
        }
    }

    fn parse_operator(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        return_type: Option<DartType>,
        start: usize,
    ) -> OperatorDecl {
        let MemberModifiers { is_external, .. } = modifiers;
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
            let op = self.cur_text().to_owned();
            self.advance();
            op
        };
        let params = self.parse_formal_param_list();
        let marker = self.parse_async_marker();
        let body = self.parse_function_body(marker);
        OperatorDecl {
            annotations,
            is_external,
            return_type,
            op,
            params,
            async_marker: marker,
            body,
            span: self.span_from(start),
        }
    }

    fn parse_factory_constructor(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        start: usize,
    ) -> ConstructorDecl {
        let MemberModifiers {
            is_external,
            is_const,
            ..
        } = modifiers;
        let keyword = self.advance(); // factory
        let (name, constructor_name) = self.parse_declared_ctor_name(&keyword);
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
            self.parse_function_body(AsyncMarker::Sync)
        };
        ConstructorDecl {
            annotations,
            is_const,
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

    /// `new(...)` / `new name(...)`: a generative constructor in Dart 3.13
    /// syntax, mapped onto the classic `C(...)` / `C.name(...)` shape.
    fn parse_new_constructor(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        start: usize,
    ) -> ConstructorDecl {
        let MemberModifiers {
            is_external,
            is_const,
            ..
        } = modifiers;
        let keyword = self.advance(); // new
        let (name, constructor_name) = self.parse_declared_ctor_name(&keyword);
        let params = self.parse_formal_param_list();
        let initializers = self.parse_constructor_initializers();
        let body = self.parse_function_body(AsyncMarker::Sync);
        ConstructorDecl {
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
        }
    }

    /// The type and constructor names after a `new`/`factory` keyword. Classic
    /// syntax spells the enclosing type (`factory C(`, `factory C.name(`); Dart
    /// 3.13 syntax omits it (`new(`, `new name(`, `factory(`, `factory name(`),
    /// in which case the type name is the enclosing type's, spanning the keyword.
    fn parse_declared_ctor_name(&mut self, keyword: &Token) -> (Identifier, Option<Identifier>) {
        if let Some(type_name) = self.ctx.ctor_name {
            let implicit = Identifier::new(type_name, Self::tok_span(keyword));
            if self.at(TokenKind::LParen) {
                return (implicit, None);
            }
            if self.cur().kind.is_identifier_like()
                && self.cur_text() != type_name
                && self.peek(1).kind != TokenKind::Dot
            {
                return (implicit, Some(self.expect_ident()));
            }
        }
        let name = self.expect_ident();
        let constructor_name = self.eat(TokenKind::Dot).map(|_| self.expect_ctor_name());
        (name, constructor_name)
    }

    /// Parse a redirecting-factory target after `=`: `Type[.name]`.
    fn parse_redirected_constructor(&mut self) -> RedirectedConstructor {
        let start = self.cur().offset();
        self.expect(TokenKind::Eq);
        let type_ = self.parse_type();
        let constructor_name = self.eat(TokenKind::Dot).map(|_| self.expect_ctor_name());
        RedirectedConstructor {
            type_,
            constructor_name,
            span: self.span_from(start),
        }
    }

    fn parse_field_or_method(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        start: usize,
    ) -> ClassMember {
        let MemberModifiers {
            is_static,
            is_abstract,
            is_external,
            is_late,
            is_final,
            is_const,
            is_async: outer_is_async,
            ..
        } = modifiers;
        let saved = self.pos;

        if is_final || is_const || is_late {
            // Const constructor: `const ClassName(` or `const ClassName.name(`.
            // Only when the enclosing type admits constructors and the name is a
            // dotted (named) form or matches the enclosing type — otherwise this
            // is a const field and falls through below.
            if is_const && self.cur().kind.is_identifier_like() && self.ctx.ctor_name.is_some() {
                let name_text = self.cur_text();
                let p1 = self.peek(1).kind;
                let dotted = p1 == TokenKind::Dot && self.is_ctor_name_at_offset(2);
                // Both the dotted (`C.named(`) and parenthesized (`C(`) forms name a
                // constructor only when the leading name is the enclosing type.
                let is_ctor =
                    self.ctx.ctor_name == Some(name_text) && (dotted || p1 == TokenKind::LParen);
                if is_ctor {
                    let ctor_saved = self.pos;
                    let name_tok = *self.cur();
                    self.advance();
                    let name = self.tok_ident(&name_tok);
                    let constructor_name =
                        self.eat(TokenKind::Dot).map(|_| self.expect_ctor_name());
                    if self.at(TokenKind::LParen) {
                        let params = self.parse_formal_param_list();
                        let initializers = self.parse_constructor_initializers();
                        let body = self.parse_function_body(AsyncMarker::Sync);
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
            let field_type = self.try_parse_type_followed_by(TokenKind::is_identifier_like);
            let name = self.expect_ident();
            return ClassMember::Field(self.finish_field(
                annotations,
                modifiers,
                field_type,
                name,
                start,
            ));
        }

        if self.is_type_start() {
            let ty = self.parse_type();

            // Typed getter: `ReturnType get name ...`
            if self.at(TokenKind::Get) && self.peek(1).kind.is_identifier_like() {
                return ClassMember::Getter(self.parse_getter(
                    annotations,
                    modifiers,
                    Some(ty),
                    start,
                ));
            }
            // Typed setter: `ReturnType set name ...`
            if self.at(TokenKind::Set) && self.peek(1).kind.is_identifier_like() {
                return ClassMember::Setter(self.parse_setter(annotations, modifiers, start));
            }
            // Typed operator: `ReturnType operator ...`
            if self.at_operator_decl() {
                return ClassMember::Operator(self.parse_operator(
                    annotations,
                    modifiers,
                    Some(ty),
                    start,
                ));
            }

            if self.cur().kind.is_identifier_like() {
                let name_tok = *self.cur();
                self.advance();
                // Method
                if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                    let name = self.tok_ident(&name_tok);
                    let type_params = self.parse_type_params();
                    let params = self.parse_formal_param_list();
                    let body_marker = self.parse_async_marker();
                    let async_marker = match (outer_is_async, body_marker) {
                        (true, AsyncMarker::Sync) => AsyncMarker::Async,
                        (true, AsyncMarker::SyncStar) => AsyncMarker::AsyncStar,
                        (_, marker) => marker,
                    };
                    let body = self.parse_function_body(body_marker);
                    let is_method_abstract = is_abstract || body.is_none();
                    return ClassMember::Method(MethodDecl {
                        annotations,
                        is_static,
                        is_abstract: is_method_abstract,
                        is_external,
                        async_marker,
                        return_type: Some(ty),
                        name,
                        type_params,
                        params,
                        body,
                        span: self.span_from(start),
                    });
                }
                let field_name = self.tok_ident(&name_tok);
                return ClassMember::Field(self.finish_field(
                    annotations,
                    modifiers,
                    Some(ty),
                    field_name,
                    start,
                ));
            }
            self.rewind_to(saved);
        }

        // No type prefix — `name(...)`, `name.ctor(...)`, or `name<T>(...)`.
        // With no return type this is a constructor only when the enclosing type
        // admits constructors AND the name is a dotted (named) form or equals the
        // enclosing type name; type parameters or an `async`/`sync*` marker (which
        // a constructor may not have) also force a method. Everything else — e.g.
        // `foo() {}` in `class C` — is an untyped method with no return type.
        if self.cur().kind.is_identifier_like() {
            let name_tok = *self.cur();
            let name_text = self.tok_text(&name_tok);
            self.advance();
            let has_ctor_dot = self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1);
            if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) || has_ctor_dot {
                let name = self.tok_ident(&name_tok);
                let constructor_name = self.eat(TokenKind::Dot).map(|_| self.expect_ctor_name());
                let type_params = self.parse_type_params();
                if self.at(TokenKind::LParen) {
                    let params = self.parse_formal_param_list();
                    let has_type_params = !type_params.is_empty();
                    let has_marker = matches!(self.cur().kind, TokenKind::Async | TokenKind::Sync);
                    // A dotted (`C.named(`) or parenthesized (`C(`) form names a
                    // constructor only when the leading name is the enclosing type —
                    // `D.named()` in `class C` is not a constructor of C.
                    let is_constructor = self.ctx.ctor_name.is_some()
                        && !has_type_params
                        && !has_marker
                        && self.ctx.ctor_name == Some(name_text);
                    if !is_constructor {
                        let body_marker = self.parse_async_marker();
                        let async_marker = match (outer_is_async, body_marker) {
                            (true, AsyncMarker::Sync) => AsyncMarker::Async,
                            (true, AsyncMarker::SyncStar) => AsyncMarker::AsyncStar,
                            (_, marker) => marker,
                        };
                        let body = self.parse_function_body(body_marker);
                        let is_method_abstract = is_abstract || body.is_none();
                        return ClassMember::Method(MethodDecl {
                            annotations,
                            is_static,
                            is_abstract: is_method_abstract,
                            is_external,
                            async_marker,
                            return_type: None,
                            name,
                            type_params,
                            params,
                            body,
                            span: self.span_from(start),
                        });
                    }
                    let initializers = self.parse_constructor_initializers();
                    let body = self.parse_function_body(AsyncMarker::Sync);
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

    fn finish_field(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: MemberModifiers,
        field_type: Option<DartType>,
        name: Identifier,
        start: usize,
    ) -> FieldDecl {
        let MemberModifiers {
            is_static,
            is_abstract,
            is_external,
            is_covariant,
            is_late,
            ..
        } = modifiers;
        let declarators = self.parse_declarators(name);
        self.expect(TokenKind::Semicolon);
        FieldDecl {
            annotations,
            is_static,
            is_abstract,
            is_external,
            is_covariant,
            is_late,
            keyword: modifiers.keyword(),
            field_type,
            declarators,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_async_marker(&mut self) -> AsyncMarker {
        match self.cur().kind {
            TokenKind::Async => {
                self.advance();
                if self.eat(TokenKind::Star).is_some() {
                    AsyncMarker::AsyncStar
                } else {
                    AsyncMarker::Async
                }
            }
            TokenKind::Sync => {
                self.advance();
                if self.eat(TokenKind::Star).is_some() {
                    AsyncMarker::SyncStar
                } else {
                    AsyncMarker::Sync
                }
            }
            _ => AsyncMarker::Sync,
        }
    }

    fn parse_constructor_initializers(&mut self) -> Vec<ConstructorInitializer> {
        if self.eat(TokenKind::Colon).is_none() {
            return Vec::new();
        }
        let mut inits = Vec::new();
        loop {
            let start = self.cur().offset();
            let init = match self.cur().kind {
                TokenKind::Super => {
                    self.advance();
                    let call_name = self.eat(TokenKind::Dot).map(|_| self.expect_ident());
                    let args = self.parse_arg_list();
                    ConstructorInitializer::SuperCall {
                        call_name,
                        args,
                        span: self.span_from(start),
                    }
                }
                TokenKind::This => {
                    self.advance();
                    let call_name = self.eat(TokenKind::Dot).map(|_| self.expect_ident());
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

    fn parse_ctor_init_value(&mut self) -> Expr {
        self.with_ctx(|ctx| ctx.in_ctor_init_value = true, Self::parse_expr)
    }

    fn parse_enum(&mut self, annotations: Vec<Annotation>, start: usize) -> EnumDecl {
        self.expect(TokenKind::Enum);
        let (name, type_params, primary_constructor) = self.parse_name_maybe_primary(false);
        let with_clause = self.parse_type_clause(TokenKind::With);
        let implements = self.parse_type_clause(TokenKind::Implements);
        let (variants, members) = if self.eat(TokenKind::Semicolon).is_some() {
            (Vec::new(), Vec::new())
        } else {
            self.expect(TokenKind::LBrace);
            let ctor_name = &self.src[name.span.start..name.span.end];
            let body = self.with_ctx(|ctx| ctx.ctor_name = Some(ctor_name), Self::parse_enum_body);
            self.expect(TokenKind::RBrace);
            body
        };
        EnumDecl {
            annotations,
            name,
            type_params,
            primary_constructor,
            with_clause,
            implements,
            variants,
            members,
            span: self.span_from(start),
        }
    }
    fn parse_enum_body(&mut self) -> (Vec<EnumVariant>, Vec<ClassMember>) {
        let mut variants = Vec::new();
        let mut members = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let v_start = self.cur().offset();
            let v_annotations = self.parse_annotations();
            if !self.cur().kind.is_identifier_like() {
                break;
            }
            let v_name = self.expect_ident();
            let type_args = self.parse_type_args();
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
            let has_comma = self.eat(TokenKind::Comma).is_some();
            if self.eat(TokenKind::Semicolon).is_some() {
                members = self.parse_class_body();
                break;
            }
            if !has_comma {
                break;
            }
        }
        (variants, members)
    }

    fn parse_extension(&mut self, annotations: Vec<Annotation>, start: usize) -> ExtensionDecl {
        self.expect(TokenKind::Extension);
        let name = if self.cur().kind.is_identifier_like() && !self.at(TokenKind::On) {
            Some(self.expect_ident())
        } else {
            None
        };
        let type_params = self.parse_type_params();
        self.eat(TokenKind::On);
        let on_type = self.parse_type();
        let members = self.parse_class_body_in(None);
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
        let (name, type_params, primary_constructor) = self.parse_name_maybe_primary(true);
        // The representation constructor is mandatory; recover with an empty one.
        let primary_constructor = primary_constructor.unwrap_or_else(|| {
            self.error("expected representation constructor");
            PrimaryConstructor {
                is_const: false,
                name_span: name.span,
                constructor_name: None,
                params: FormalParamList {
                    params: Vec::new(),
                    span: self.cur_span(),
                },
                span: self.cur_span(),
            }
        });
        let implements = self.parse_type_clause(TokenKind::Implements);
        let members = self.parse_class_body_in(Some(&self.src[name.span.start..name.span.end]));
        ExtensionTypeDecl {
            annotations,
            name,
            type_params,
            primary_constructor,
            implements,
            members,
            span: self.span_from(start),
        }
    }

    fn parse_typedef(&mut self, annotations: Vec<Annotation>, start: usize) -> TypeAliasDecl {
        self.expect(TokenKind::Typedef);
        let saved = self.pos;

        // Modern form: `typedef Name<T> = Type;`
        if self.cur().kind.is_identifier_like() {
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
                    legacy_params: None,
                    span: self.span_from(start),
                };
            }
            self.rewind_to(saved);
        }

        // Legacy form: `typedef [returnType] Name<T>(params);` — synthesize a
        // function type from the return type and parameter list.
        let leading = self.parse_type();
        let ty_start = leading.span().start;
        let (return_type, name, type_params) = if self.cur().kind.is_identifier_like() {
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
        let fn_span = Span::new(ty_start, params.span.end);
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
            legacy_params: Some(params),
            span: self.span_from(start),
        }
    }

    /// Split a parsed named type into an alias head: its name and any type
    /// arguments reinterpreted as type parameters (`Comparator<T>` → `Comparator`,
    /// `[T]`). Used for legacy typedefs that lack a return type.
    fn named_type_to_alias_head(ty: DartType) -> (Identifier, Vec<TypeParam>) {
        match ty {
            DartType::Named(n) => {
                let span = n.span;
                let name = n
                    .segments
                    .into_iter()
                    .next_back()
                    .unwrap_or_else(|| Identifier::new("<error>", span));
                let type_params = n
                    .type_args
                    .into_iter()
                    .map(|arg| {
                        let sp = *arg.span();
                        let tp_name = match arg {
                            DartType::Named(mut a) => a
                                .segments
                                .pop()
                                .unwrap_or_else(|| Identifier::new("<error>", sp)),
                            other => Identifier::new("<error>", *other.span()),
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
            other => (Identifier::new("<error>", *other.span()), Vec::new()),
        }
    }

    /// Convert a formal parameter list into function-type parameters for a
    /// synthesized function type (legacy typedefs, etc.).
    fn formals_to_function_type_params(params: &FormalParamList) -> Vec<FunctionTypeParam> {
        params
            .params
            .iter()
            .map(|p| FunctionTypeParam {
                name: Some(p.name.clone()),
                param_type: p
                    .param_type
                    .clone()
                    .unwrap_or(DartType::Dynamic { span: p.span }),
                is_required: matches!(
                    p.kind,
                    ParamKind::Positional | ParamKind::Named { required: true }
                ),
                is_named: matches!(p.kind, ParamKind::Named { .. }),
            })
            .collect()
    }

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
        let at_untyped_accessor = self.at_any(&[TokenKind::Get, TokenKind::Set])
            && self.peek(1).kind.is_identifier_like();
        let return_type = if !at_untyped_accessor {
            self.try_parse_type_followed_by(TokenKind::is_identifier_like)
        } else {
            None
        };

        // Top-level getter: `(ReturnType) get name { ... }`
        if self.at(TokenKind::Get) && self.peek(1).kind.is_identifier_like() {
            self.advance(); // get
            let name = self.expect_ident();
            let async_marker = self.parse_async_marker();
            let body = self.parse_function_body(async_marker);
            return Some(TopLevelDecl::Function(FunctionDecl {
                annotations,
                is_external,
                async_marker,
                kind: FunctionKind::Getter,
                return_type,
                name,
                type_params: vec![],
                body,
                span: self.span_from(start),
            }));
        }

        // Top-level setter: `(ReturnType) set name(param) { ... }`
        if self.at(TokenKind::Set) && self.peek(1).kind.is_identifier_like() {
            self.advance(); // set
            let name = self.expect_ident();
            let params = self.parse_formal_param_list();
            let async_marker = self.parse_async_marker();
            let body = self.parse_function_body(async_marker);
            return Some(TopLevelDecl::Function(FunctionDecl {
                annotations,
                is_external,
                async_marker,
                kind: FunctionKind::Setter(params),
                return_type,
                name,
                type_params: vec![],
                body,
                span: self.span_from(start),
            }));
        }

        if !self.cur().kind.is_identifier_like() {
            return None;
        }
        let name = self.expect_ident();

        if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
            let type_params = self.parse_type_params();
            let params = self.parse_formal_param_list();
            let async_marker = self.parse_async_marker();
            let body = self.parse_function_body(async_marker);
            return Some(TopLevelDecl::Function(FunctionDecl {
                annotations,
                is_external,
                async_marker,
                kind: FunctionKind::Function(params),
                return_type,
                name,
                type_params,
                body,
                span: self.span_from(start),
            }));
        }

        let declarators = self.parse_declarators(name);
        self.eat(TokenKind::Semicolon);
        Some(TopLevelDecl::Variable(TopLevelVarDecl {
            annotations,
            is_external,
            keyword: Self::var_keyword(is_const, is_final),
            is_late,
            var_type: return_type,
            declarators,
            span: self.span_from(start),
        }))
    }

    pub(super) fn parse_string_lit(&mut self) -> StringLitNode {
        let tok = *self.cur();
        let raw = self.tok_text(&tok);
        let span = Self::tok_span(&tok);
        let is_string_lit = matches!(tok.kind, TokenKind::StringLit);
        if is_string_lit {
            self.advance();
        } else {
            self.error("expected string literal");
        }
        let interpolations = if is_string_lit {
            let (interps, interp_errors) =
                scan_interpolations(self.syntax, self.src, tok.offset(), raw, self.version);
            // Surface any interpolation sub-parse diagnostics to the enclosing
            // program (their offsets are already absolute).
            self.errors.extend(interp_errors);
            interps
        } else {
            Vec::new()
        };
        let mut node = StringLitNode {
            raw: raw.to_owned(),
            is_adjacent: false,
            segment_spans: Vec::new(),
            has_raw_segment: raw.starts_with('r'),
            value: String::new(),
            span,
            interpolations,
        };
        node.value = node
            .delimiter()
            .map_or_else(|| raw.strip_prefix('r').unwrap_or(raw), |lit| lit.content)
            .to_owned();
        node
    }
}

/// Scan a single string literal's raw text for interpolation regions and parse
/// each into an expression with an absolute source span.
///
/// `lit_start` is the absolute byte offset of `raw` within `src`. Raw strings
/// (`r'...'`) yield no interpolations. A `${expr}` whose inner slice fails to
/// parse cleanly records no interpolation, but its parse diagnostics (with
/// absolute source offsets) are returned alongside so the enclosing program can
/// surface them.
fn scan_interpolations(
    syntax: &SyntaxStore,
    src: &str,
    lit_start: usize,
    raw: &str,
    version: LanguageVersion,
) -> (Vec<StringInterpolation>, Vec<ParseError>) {
    let mut out = Vec::new();
    let mut errs = Vec::new();
    visit_interpolations(raw, lit_start, |region| match region {
        InterpolationRegion::Expression(span) => {
            let (interp, interp_errs) = parse_interp_expr(syntax, src, span, version);
            if let Some(interp) = interp {
                out.push(interp);
            }
            errs.extend(interp_errs);
        }
        InterpolationRegion::Identifier(span) => out.push(StringInterpolation {
            expr: if &src[span.start..span.end] == "this" {
                Expr::This { span }
            } else {
                Expr::Ident(Identifier::new(&src[span.start..span.end], span))
            },
            span,
        }),
    });
    (out, errs)
}

/// Parse a retained interpolation stream as one standalone expression. Invalid
/// expressions keep their absolute diagnostics but do not become interpolations.
fn parse_interp_expr(
    syntax: &SyntaxStore,
    src: &str,
    span: Span,
    version: LanguageVersion,
) -> (Option<StringInterpolation>, Vec<ParseError>) {
    if span.start >= span.end {
        return (None, Vec::new());
    }
    let retained = syntax
        .interpolation(span)
        .expect("retained interpolation stream");
    let mut sub = Parser::new(retained, src, version);
    let expr = sub.parse_expr();
    if !sub.errors.is_empty() || !sub.at(TokenKind::Eof) || matches!(expr, Expr::Error { .. }) {
        return (None, sub.errors);
    }
    (Some(StringInterpolation { expr, span }), Vec::new())
}
