use falcon_syntax::Feature;
use falcon_syntax::cst::{AsyncMarker, NodeKind};
use falcon_syntax::token::TokenKind;

use super::{CompletedMarker, Marker, Parser};

#[derive(Clone, Copy, Default)]
struct MemberModifiers {
    is_late: bool,
    is_final: bool,
    is_const: bool,
}

impl<'src> Parser<'src> {
    pub(super) fn parse_library_directive(&mut self, marker: Marker) -> CompletedMarker {
        self.advance(); // library
        // Dart 3 allows bare `library;` with no name.
        if self.cur().kind.is_identifier_like() {
            self.expect_ident();
            while self.eat(TokenKind::Dot).is_some() {
                self.expect_ident();
            }
        }
        self.expect(TokenKind::Semicolon);
        marker.complete(self, NodeKind::LibraryDirective)
    }

    pub(super) fn parse_part_of(&mut self, marker: Marker) -> CompletedMarker {
        self.advance(); // part
        self.advance(); // of
        if matches!(self.cur().kind, TokenKind::StringLit) {
            self.parse_adjacent_strings();
        } else {
            self.expect_ident();
            while self.eat(TokenKind::Dot).is_some() {
                self.expect_ident();
            }
        }
        self.expect(TokenKind::Semicolon);
        marker.complete(self, NodeKind::PartOfDirective)
    }

    pub(super) fn parse_part(&mut self, marker: Marker) -> CompletedMarker {
        self.expect(TokenKind::Part);
        if !matches!(self.cur().kind, TokenKind::StringLit) {
            self.error("expected string literal in part directive");
            self.missing(NodeKind::Error);
            return marker.complete(self, NodeKind::Error);
        }
        self.parse_adjacent_strings();
        self.expect(TokenKind::Semicolon);
        marker.complete(self, NodeKind::PartDirective)
    }

    pub(super) fn parse_import(&mut self, marker: Marker) -> CompletedMarker {
        self.expect(TokenKind::Import);
        self.parse_adjacent_strings();
        self.parse_configurable_uris();
        self.eat(TokenKind::Deferred);
        if self.eat(TokenKind::As).is_some() {
            self.expect_ident();
        }
        self.parse_import_combinators();
        self.expect(TokenKind::Semicolon);
        marker.complete(self, NodeKind::ImportDirective)
    }

    pub(super) fn parse_export(&mut self, marker: Marker) -> CompletedMarker {
        self.expect(TokenKind::Export);
        self.parse_adjacent_strings();
        self.parse_configurable_uris();
        self.parse_import_combinators();
        self.expect(TokenKind::Semicolon);
        marker.complete(self, NodeKind::ExportDirective)
    }

    /// Parse zero or more configurable-URI clauses:
    /// `if (dotted.name [== 'value']) 'uri'`.
    fn parse_configurable_uris(&mut self) {
        while self.at(TokenKind::If) {
            let marker = self.start();
            self.advance(); // if
            self.expect(TokenKind::LParen);
            self.expect_ident();
            while self.eat(TokenKind::Dot).is_some() {
                self.expect_ident();
            }
            if self.eat(TokenKind::EqEq).is_some() {
                self.parse_string_lit();
            }
            self.expect(TokenKind::RParen);
            self.parse_adjacent_strings();
            marker.complete(self, NodeKind::ConfigurableUri);
        }
    }

    fn parse_import_combinators(&mut self) {
        loop {
            if self.at(TokenKind::Show) || self.at(TokenKind::Hide) {
                let marker = self.start();
                self.advance();
                self.parse_ident_list();
                marker.complete(self, NodeKind::Combinator);
            } else {
                break;
            }
        }
    }

    fn parse_ident_list(&mut self) {
        self.expect_ident();
        while self.eat(TokenKind::Comma).is_some() {
            if self.cur().kind.is_identifier_like() {
                self.expect_ident();
            }
        }
    }

    pub(super) fn parse_top_level_decl(&mut self) -> Option<NodeKind> {
        let mut is_final = false;
        loop {
            match self.cur().kind {
                TokenKind::Abstract => {
                    self.advance();
                }
                TokenKind::Interface | TokenKind::Base | TokenKind::Sealed => {
                    is_final = false;
                    self.advance();
                }
                TokenKind::Final => {
                    is_final = true;
                    self.advance();
                }
                // `static` is only legal on class members. A following `(`/`<`
                // instead makes it the name of a top-level function.
                TokenKind::Static
                    if !matches!(self.peek(1).kind, TokenKind::LParen | TokenKind::Lt) =>
                {
                    self.error("Can't have modifier 'static' here");
                    self.advance();
                }
                _ => break,
            }
        }
        Some(match self.cur().kind {
            TokenKind::Class => self.parse_class_or_type_alias(),
            TokenKind::Mixin => {
                if self.peek(1).kind == TokenKind::Class {
                    self.advance(); // mixin
                    self.parse_mixin_class();
                    NodeKind::MixinClassDecl
                } else {
                    self.parse_mixin();
                    NodeKind::MixinDecl
                }
            }
            TokenKind::Enum => {
                self.parse_enum();
                NodeKind::EnumDecl
            }
            TokenKind::Extension => {
                if self.peek(1).kind == TokenKind::Type {
                    self.parse_extension_type();
                    NodeKind::ExtensionTypeDecl
                } else {
                    self.parse_extension();
                    NodeKind::ExtensionDecl
                }
            }
            TokenKind::Typedef => {
                self.parse_typedef();
                NodeKind::TypeAliasDecl
            }
            _ => return self.try_parse_top_level_func_or_var(is_final),
        })
    }

    fn parse_class_or_type_alias(&mut self) -> NodeKind {
        self.expect(TokenKind::Class);
        let (name, has_primary_constructor) = self.parse_name_maybe_primary(false);
        // Mixin-application class: `class MA = Super with M [implements I];`
        if !has_primary_constructor && self.eat(TokenKind::Eq).is_some() {
            let superclass_marker = self.start();
            self.parse_type();
            superclass_marker.complete(self, NodeKind::ExtendsClause);
            let with_marker = self.start();
            self.expect(TokenKind::With);
            self.parse_type_list();
            with_marker.complete(self, NodeKind::WithClause);
            self.parse_type_clause(TokenKind::Implements);
            self.expect(TokenKind::Semicolon);
            return NodeKind::ClassTypeAliasDecl;
        }
        self.parse_extends_clause();
        self.parse_type_clause(TokenKind::With);
        self.parse_type_clause(TokenKind::Implements);
        self.parse_class_body_in(Some(name.text(self)));
        NodeKind::ClassDecl
    }

    /// `<classNameMaybePrimary>`: `[const] Name<T>`, optionally followed by a
    /// Dart 3.13 primary constructor `[.name](params)`.
    fn parse_name_maybe_primary(&mut self, representation: bool) -> (CompletedMarker, bool) {
        let is_const = self.eat(TokenKind::Const).is_some();
        let name = self.expect_ident();
        self.parse_type_params();
        let named = self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1);
        if !(is_const || named || self.at(TokenKind::LParen)) {
            return (name, false);
        }
        if !representation && !self.version.supports(Feature::PrimaryConstructors) {
            self.feature_error("This requires the 'primary-constructors' language feature");
        }
        let marker = self.start();
        if named {
            self.advance(); // .
            self.expect_ctor_name();
        }
        self.parse_formal_param_list();
        marker.complete(self, NodeKind::PrimaryConstructor);
        (name, true)
    }

    fn parse_mixin(&mut self) {
        self.expect(TokenKind::Mixin);
        self.expect_ident();
        self.parse_type_params();
        self.parse_type_clause(TokenKind::On);
        self.parse_type_clause(TokenKind::Implements);
        self.parse_class_body_in(None);
    }

    fn parse_mixin_class(&mut self) {
        self.expect(TokenKind::Class);
        let (name, _) = self.parse_name_maybe_primary(false);
        self.parse_extends_clause();
        self.parse_type_clause(TokenKind::With);
        self.parse_type_clause(TokenKind::Implements);
        self.parse_class_body_in(Some(name.text(self)));
    }

    fn parse_extends_clause(&mut self) {
        if !self.at(TokenKind::Extends) {
            return;
        }
        let marker = self.start();
        self.advance();
        self.parse_type();
        marker.complete(self, NodeKind::ExtendsClause);
    }

    fn parse_type_clause(&mut self, keyword: TokenKind) {
        if self.at(keyword) {
            let marker = self.start();
            self.advance();
            self.parse_type_list();
            let kind = match keyword {
                TokenKind::With => NodeKind::WithClause,
                TokenKind::Implements => NodeKind::ImplementsClause,
                TokenKind::On => NodeKind::OnClause,
                _ => unreachable!("type clause keyword"),
            };
            marker.complete(self, kind);
        }
    }

    fn parse_type_list(&mut self) {
        self.parse_type();
        while self.eat(TokenKind::Comma).is_some() {
            if self.is_type_start() {
                self.parse_type();
            }
        }
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

    fn parse_class_body(&mut self) {
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            self.parse_class_member();
        }
    }

    /// Parse a braced class body with the enclosing-constructor-name context
    /// set (and restored afterwards). `ctor_name` is `Some(type name)` when the
    /// enclosing declaration may declare constructors, or `None` for
    /// mixins/extensions whose untyped members are always methods. Dart 3.13
    /// also accepts `;` for an empty body (`abstract class Marker;`).
    fn parse_class_body_in(&mut self, ctor_name: Option<&'src str>) {
        if let Some(semicolon) = self.eat(TokenKind::Semicolon) {
            if !self.version.supports(Feature::PrimaryConstructors) {
                self.feature_error_at(
                    semicolon.offset(),
                    "This requires the 'primary-constructors' language feature",
                );
            }
            return;
        }
        self.expect(TokenKind::LBrace);
        self.with_ctx(|ctx| ctx.ctor_name = ctor_name, Self::parse_class_body);
        self.expect(TokenKind::RBrace);
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
                    self.advance();
                }
                TokenKind::Abstract => {
                    self.advance();
                }
                TokenKind::External => {
                    self.advance();
                }
                TokenKind::Covariant => {
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

    fn parse_class_member(&mut self) -> CompletedMarker {
        let marker = self.start();
        let kind = self.parse_class_member_inner();
        marker.complete(self, kind)
    }

    fn parse_class_member_inner(&mut self) -> NodeKind {
        self.parse_annotations();
        let modifiers = self.parse_member_modifiers();
        if self.at(TokenKind::Get) && self.peek(1).kind.is_identifier_like() {
            self.parse_getter();
            return NodeKind::GetterDecl;
        }
        if self.at(TokenKind::Set) && self.peek(1).kind.is_identifier_like() {
            self.parse_setter();
            return NodeKind::SetterDecl;
        }
        if self.at_operator_decl() {
            self.parse_operator();
            return NodeKind::OperatorDecl;
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
                self.feature_error("This requires the 'primary-constructors' language feature");
            }
            self.advance();
            self.parse_constructor_initializers();
            self.parse_function_body(AsyncMarker::Sync);
            return NodeKind::PrimaryConstructorBody;
        }
        // Dart 3.13 generative constructor syntax: `new(...)`, `new name(...)`.
        if self.at(TokenKind::New) && self.ctx.ctor_name.is_some() {
            if !self.version.supports(Feature::PrimaryConstructors) {
                self.feature_error("This requires the 'primary-constructors' language feature");
            }
            self.parse_new_constructor();
            return NodeKind::ConstructorDecl;
        }
        // `factory` can also be a plain member name.
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
            self.parse_factory_constructor();
            return NodeKind::ConstructorDecl;
        }
        if self.eat(TokenKind::Var).is_some() {
            let name = self.expect_ident();
            self.finish_field(name);
            return NodeKind::FieldDecl;
        }
        self.parse_field_or_method(modifiers)
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

    pub(super) fn expect_ctor_name(&mut self) -> CompletedMarker {
        if self.at(TokenKind::New) {
            let marker = self.start();
            self.advance();
            return marker.complete(self, NodeKind::Identifier);
        }
        self.expect_ident()
    }

    fn parse_getter(&mut self) {
        self.advance(); // get
        self.expect_ident();
        let async_marker = self.parse_async_marker();
        self.parse_function_body(async_marker);
    }

    fn parse_setter(&mut self) {
        self.advance(); // set
        self.expect_ident();
        self.expect(TokenKind::LParen);
        // A setter's single value parameter accepts the same leading modifiers
        // and trailing comma as other formal parameters.
        let parameter_marker = self.start();
        self.parse_annotations();
        self.eat(TokenKind::Covariant);
        self.eat(TokenKind::Final);
        self.eat(TokenKind::Var);
        if self.peek(1).kind != TokenKind::RParen {
            self.try_parse_type_followed_by(TokenKind::is_identifier_like);
        }
        self.expect_ident();
        self.parse_type_params();
        if self.at(TokenKind::LParen) {
            self.parse_formal_param_list();
            self.eat(TokenKind::Qmark);
        }
        parameter_marker.complete(self, NodeKind::SetterParameter);
        self.eat(TokenKind::Comma); // optional trailing comma
        self.expect(TokenKind::RParen);
        self.parse_function_body(AsyncMarker::Sync);
    }

    fn parse_operator(&mut self) {
        self.advance(); // operator
        // `[]` and `[]=` are lexed as separate bracket tokens.
        if self.at(TokenKind::LBracket) {
            self.advance(); // [
            self.expect(TokenKind::RBracket); // ]
            self.eat(TokenKind::Eq);
        } else {
            self.advance();
        }
        self.parse_formal_param_list();
        let marker = self.parse_async_marker();
        self.parse_function_body(marker);
    }

    fn parse_factory_constructor(&mut self) {
        self.advance(); // factory
        self.parse_declared_ctor_name();
        self.parse_formal_param_list();
        // Redirecting factory: `factory C() = D;` / `= D.named;` / `= D<int>;`.
        if self.at(TokenKind::Eq) {
            self.parse_redirected_constructor();
            self.expect(TokenKind::Semicolon);
        } else {
            self.parse_function_body(AsyncMarker::Sync);
        }
    }

    /// `new(...)` / `new name(...)`: a generative constructor in Dart 3.13.
    fn parse_new_constructor(&mut self) {
        self.advance(); // new
        self.parse_declared_ctor_name();
        self.parse_formal_param_list();
        self.parse_constructor_initializers();
        self.parse_function_body(AsyncMarker::Sync);
    }

    /// Parse an explicit type/constructor name or the Dart 3.13 implicit form.
    fn parse_declared_ctor_name(&mut self) {
        if let Some(type_name) = self.ctx.ctor_name {
            if self.at(TokenKind::LParen) {
                return;
            }
            if self.cur().kind.is_identifier_like()
                && self.cur_text() != type_name
                && self.peek(1).kind != TokenKind::Dot
            {
                self.expect_ident();
                return;
            }
        }
        self.expect_ident();
        if self.eat(TokenKind::Dot).is_some() {
            self.expect_ctor_name();
        }
    }

    /// Parse a redirecting-factory target after `=`: `Type[.name]`.
    fn parse_redirected_constructor(&mut self) -> CompletedMarker {
        let marker = self.start();
        self.expect(TokenKind::Eq);
        self.parse_type();
        if self.eat(TokenKind::Dot).is_some() {
            self.expect_ctor_name();
        }
        marker.complete(self, NodeKind::RedirectedConstructor)
    }

    fn parse_field_or_method(&mut self, modifiers: MemberModifiers) -> NodeKind {
        let MemberModifiers {
            is_late,
            is_final,
            is_const,
        } = modifiers;
        let saved = self.checkpoint();
        if is_final || is_const || is_late {
            return self.parse_modified_field_or_constructor(is_const);
        }

        if self.is_type_start() {
            self.parse_type();
            if self.at(TokenKind::Get) && self.peek(1).kind.is_identifier_like() {
                self.parse_getter();
                return NodeKind::GetterDecl;
            }
            if self.at(TokenKind::Set) && self.peek(1).kind.is_identifier_like() {
                self.parse_setter();
                return NodeKind::SetterDecl;
            }
            if self.at_operator_decl() {
                self.parse_operator();
                return NodeKind::OperatorDecl;
            }
            if self.cur().kind.is_identifier_like() {
                let name_marker = self.start();
                self.advance();
                let name = name_marker.complete(self, NodeKind::Identifier);
                if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
                    self.parse_type_params();
                    self.parse_formal_param_list();
                    let body_marker = self.parse_async_marker();
                    self.parse_function_body(body_marker);
                    return NodeKind::MethodDecl;
                }
                self.finish_field(name);
                return NodeKind::FieldDecl;
            }
            self.restore(saved);
        }

        // Type parameters or an async/sync marker force an untyped method.
        if self.cur().kind.is_identifier_like() {
            let name_text = self.cur().text(self.src);
            let name_marker = self.start();
            self.advance();
            name_marker.complete(self, NodeKind::Identifier);
            let has_ctor_dot = self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1);
            if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) || has_ctor_dot {
                if self.eat(TokenKind::Dot).is_some() {
                    self.expect_ctor_name();
                }
                let type_params = self.parse_type_params();
                if self.at(TokenKind::LParen) {
                    self.parse_formal_param_list();
                    let has_type_params = type_params != 0;
                    let has_marker = matches!(self.cur().kind, TokenKind::Async | TokenKind::Sync);
                    let is_constructor = self.ctx.ctor_name.is_some()
                        && !has_type_params
                        && !has_marker
                        && self.ctx.ctor_name == Some(name_text);
                    if !is_constructor {
                        let body_marker = self.parse_async_marker();
                        self.parse_function_body(body_marker);
                        return NodeKind::MethodDecl;
                    }
                    self.parse_constructor_initializers();
                    self.parse_function_body(AsyncMarker::Sync);
                    return NodeKind::ConstructorDecl;
                }
            }
            self.restore(saved);
        }
        // Give up — emit error node and skip to next member.
        let recovery = self.start();
        self.missing(NodeKind::Identifier);
        self.error("could not parse class member");
        self.synchronize(&[TokenKind::Semicolon, TokenKind::RBrace]);
        self.eat(TokenKind::Semicolon);
        recovery.complete(self, NodeKind::Error);
        NodeKind::ErrorNode
    }

    fn parse_modified_field_or_constructor(&mut self, is_const: bool) -> NodeKind {
        // Only the enclosing type's name can introduce a const constructor.
        if is_const && self.cur().kind.is_identifier_like() && self.ctx.ctor_name.is_some() {
            let name_text = self.cur_text();
            let p1 = self.peek(1).kind;
            let dotted = p1 == TokenKind::Dot && self.is_ctor_name_at_offset(2);
            let is_ctor =
                self.ctx.ctor_name == Some(name_text) && (dotted || p1 == TokenKind::LParen);
            if is_ctor {
                let ctor_saved = self.checkpoint();
                let name_marker = self.start();
                self.advance();
                name_marker.complete(self, NodeKind::Identifier);
                if self.eat(TokenKind::Dot).is_some() {
                    self.expect_ctor_name();
                }
                if self.at(TokenKind::LParen) {
                    self.parse_formal_param_list();
                    self.parse_constructor_initializers();
                    self.parse_function_body(AsyncMarker::Sync);
                    return NodeKind::ConstructorDecl;
                }
                self.restore(ctor_saved);
            }
        }
        self.try_parse_type_followed_by(TokenKind::is_identifier_like);
        let name = self.expect_ident();
        self.finish_field(name);
        NodeKind::FieldDecl
    }

    fn finish_field(&mut self, name: CompletedMarker) {
        self.parse_declarators(name);
        self.expect(TokenKind::Semicolon);
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

    fn parse_constructor_initializers(&mut self) {
        if self.eat(TokenKind::Colon).is_none() {
            return;
        }
        loop {
            let marker = self.start();
            let kind = match self.cur().kind {
                TokenKind::Super => {
                    self.advance();
                    if self.eat(TokenKind::Dot).is_some() {
                        self.expect_ident();
                    }
                    self.parse_arg_list();
                    NodeKind::SuperCallInitializer
                }
                TokenKind::This => {
                    self.advance();
                    let call_name = self.eat(TokenKind::Dot).map(|_| self.expect_ident());
                    if self.at(TokenKind::LParen) {
                        self.parse_arg_list();
                        NodeKind::ThisCallInitializer
                    } else {
                        // this.field = value
                        if call_name.is_none() {
                            self.missing(NodeKind::Identifier);
                        }
                        self.expect(TokenKind::Eq);
                        self.parse_ctor_init_value();
                        NodeKind::FieldInitializer
                    }
                }
                TokenKind::Assert => {
                    self.advance();
                    self.expect(TokenKind::LParen);
                    self.parse_expr();
                    if self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
                        self.parse_expr();
                        self.eat(TokenKind::Comma); // optional trailing comma
                    }
                    self.expect(TokenKind::RParen);
                    NodeKind::AssertInitializer
                }
                _ => {
                    // field = value
                    self.expect_ident();
                    self.expect(TokenKind::Eq);
                    self.parse_ctor_init_value();
                    NodeKind::FieldInitializer
                }
            };
            marker.complete(self, kind);
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
    }

    fn parse_ctor_init_value(&mut self) -> CompletedMarker {
        self.with_ctx(|ctx| ctx.in_ctor_init_value = true, Self::parse_expr)
    }

    fn parse_enum(&mut self) {
        self.expect(TokenKind::Enum);
        let (name, _) = self.parse_name_maybe_primary(false);
        self.parse_type_clause(TokenKind::With);
        self.parse_type_clause(TokenKind::Implements);
        if self.eat(TokenKind::Semicolon).is_none() {
            self.expect(TokenKind::LBrace);
            let ctor_name = name.text(self);
            self.with_ctx(|ctx| ctx.ctor_name = Some(ctor_name), Self::parse_enum_body);
            self.expect(TokenKind::RBrace);
        }
    }

    fn parse_enum_body(&mut self) {
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let marker = self.start();
            self.parse_annotations();
            if !self.cur().kind.is_identifier_like() {
                marker.complete(self, NodeKind::Error);
                break;
            }
            self.expect_ident();
            self.parse_type_args();
            // Enhanced-enum constant invoking a named (or `.new`) constructor.
            if self.at(TokenKind::Dot) && self.is_ctor_name_at_offset(1) {
                self.advance(); // .
                self.expect_ctor_name();
            }
            if self.at(TokenKind::LParen) {
                self.parse_arg_list();
            }
            marker.complete(self, NodeKind::EnumVariant);
            let has_comma = self.eat(TokenKind::Comma).is_some();
            if self.eat(TokenKind::Semicolon).is_some() {
                self.parse_class_body();
                break;
            }
            if !has_comma {
                break;
            }
        }
    }

    fn parse_extension(&mut self) {
        self.expect(TokenKind::Extension);
        if self.cur().kind.is_identifier_like() && !self.at(TokenKind::On) {
            self.expect_ident();
        }
        self.parse_type_params();
        let on_marker = self.start();
        self.eat(TokenKind::On);
        self.parse_type();
        on_marker.complete(self, NodeKind::OnClause);
        self.parse_class_body_in(None);
    }

    fn parse_extension_type(&mut self) {
        self.expect(TokenKind::Extension);
        self.expect(TokenKind::Type);
        let (name, has_primary_constructor) = self.parse_name_maybe_primary(true);
        // The representation constructor is mandatory; recover with an empty one.
        if !has_primary_constructor {
            self.error("expected representation constructor");
            let marker = self.start();
            self.missing(NodeKind::Identifier);
            let params_marker = self.start();
            self.missing(NodeKind::Identifier);
            params_marker.complete(self, NodeKind::FormalParamList);
            marker.complete(self, NodeKind::PrimaryConstructor);
        }
        self.parse_type_clause(TokenKind::Implements);
        self.parse_class_body_in(Some(name.text(self)));
    }

    fn parse_typedef(&mut self) {
        self.expect(TokenKind::Typedef);
        let saved = self.checkpoint();
        // Modern form: `typedef Name<T> = Type;`
        if self.cur().kind.is_identifier_like() {
            self.expect_ident();
            self.parse_type_params();
            if self.eat(TokenKind::Eq).is_some() {
                self.parse_type();
                self.expect(TokenKind::Semicolon);
                return;
            }
            self.restore(saved);
        }
        // Legacy form: `typedef [returnType] Name<T>(params);`
        let legacy_marker = self.start();
        let return_type = self.checkpoint();
        self.parse_type();
        if !self.cur().kind.is_identifier_like() {
            // An omitted return type leaves the alias name itself looking like
            // a type. Reparse it as a declaration name, not a phantom NamedType.
            self.restore(return_type);
        }
        self.expect_ident();
        self.parse_type_params();
        self.parse_formal_param_list();
        legacy_marker.complete(self, NodeKind::LegacyTypedef);
        self.expect(TokenKind::Semicolon);
    }

    fn try_parse_top_level_func_or_var(&mut self, outer_is_final: bool) -> Option<NodeKind> {
        self.eat(TokenKind::External);
        // `final` may already have been consumed by the outer modifier loop.
        self.eat(TokenKind::Late);
        if !outer_is_final {
            self.eat(TokenKind::Final);
        }
        self.eat(TokenKind::Const);
        self.eat(TokenKind::Var);
        // Bare get/set before an accessor name is not a return type.
        let at_untyped_accessor = self.at_any(&[TokenKind::Get, TokenKind::Set])
            && self.peek(1).kind.is_identifier_like();
        if !at_untyped_accessor {
            self.try_parse_type_followed_by(TokenKind::is_identifier_like);
        }
        if self.at(TokenKind::Get) && self.peek(1).kind.is_identifier_like() {
            self.advance(); // get
            self.expect_ident();
            let async_marker = self.parse_async_marker();
            self.parse_function_body(async_marker);
            return Some(NodeKind::FunctionDecl);
        }
        if self.at(TokenKind::Set) && self.peek(1).kind.is_identifier_like() {
            self.advance(); // set
            self.expect_ident();
            self.parse_formal_param_list();
            let async_marker = self.parse_async_marker();
            self.parse_function_body(async_marker);
            return Some(NodeKind::FunctionDecl);
        }
        if !self.cur().kind.is_identifier_like() {
            return None;
        }
        let name = self.expect_ident();
        if self.at(TokenKind::LParen) || self.at(TokenKind::Lt) {
            self.parse_type_params();
            self.parse_formal_param_list();
            let async_marker = self.parse_async_marker();
            self.parse_function_body(async_marker);
            return Some(NodeKind::FunctionDecl);
        }
        self.parse_declarators(name);
        self.eat(TokenKind::Semicolon);
        Some(NodeKind::TopLevelVarDecl)
    }

    pub(super) fn parse_string_lit(&mut self) -> CompletedMarker {
        let marker = self.start();
        let tok = *self.cur();
        if tok.kind == TokenKind::StringLit {
            self.advance();
            self.scan_string_interpolations(&tok);
        } else {
            self.error("expected string literal");
            self.missing(NodeKind::Error);
        }
        marker.complete(self, NodeKind::StringLitNode)
    }
}
