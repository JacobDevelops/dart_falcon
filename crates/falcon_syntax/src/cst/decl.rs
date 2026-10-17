use super::macros::DebugList;
use super::*;
use crate::token::TokenKind;

fn child<'a, T: AstNode<'a>>(node: SyntaxNode<'a>) -> Option<T> {
    node.children().find_map(T::cast)
}
fn required<'a, T: AstNode<'a>>(node: SyntaxNode<'a>) -> T {
    child(node).expect("parser emits required declaration child")
}
fn names(node: SyntaxNode<'_>) -> impl DoubleEndedIterator<Item = Identifier<'_>> + Clone {
    node.children().filter_map(Identifier::from_node)
}
fn name(node: SyntaxNode<'_>) -> Identifier<'_> {
    names(node)
        .next()
        .expect("parser emits declaration identifier")
}
fn has(node: SyntaxNode<'_>, kind: TokenKind) -> bool {
    node.child_token(kind).is_some()
}
fn assigned_value(node: SyntaxNode<'_>) -> Option<Expr<'_>> {
    let end = node
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|token| matches!(token.kind(), TokenKind::Eq | TokenKind::Colon))?
        .span()
        .end;
    node.children()
        .filter(|child| child.span().start >= end)
        .find_map(Expr::cast)
}
fn clause<'a, T: AstNode<'a>>(node: SyntaxNode<'a>, kind: NodeKind) -> AstChildren<'a, T> {
    let mut children = node.children();
    if let Some(role) = children.find(|child| child.kind() == kind) {
        return AstChildren::new(role);
    }
    AstChildren::from_children(children)
}
fn restriction(node: SyntaxNode<'_>) -> Option<ClassRestriction> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter_map(|token| match token.kind() {
            TokenKind::Base => Some(ClassRestriction::Base),
            TokenKind::Interface => Some(ClassRestriction::Interface),
            TokenKind::Final => Some(ClassRestriction::Final),
            TokenKind::Sealed => Some(ClassRestriction::Sealed),
            _ => None,
        })
        .next_back()
}
fn keyword(node: SyntaxNode<'_>) -> VarKeyword {
    if has(node, TokenKind::Const) {
        VarKeyword::Const
    } else if has(node, TokenKind::Final) {
        VarKeyword::Final
    } else {
        VarKeyword::Var
    }
}
fn asynchronous(node: SyntaxNode<'_>) -> AsyncMarker {
    let mut marker = AsyncMarker::Sync;
    let cutoff = match node.kind() {
        NodeKind::GetterDecl => name(node).span().end,
        NodeKind::OperatorDecl => required::<FormalParamList>(node).span().end,
        _ => 0,
    };
    for token in node
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|token| token.span().start >= cutoff)
    {
        let star = token
            .next_significant()
            .is_some_and(|next| next.kind() == TokenKind::Star);
        match token.kind() {
            TokenKind::Async => {
                marker = if star {
                    AsyncMarker::AsyncStar
                } else {
                    AsyncMarker::Async
                }
            }
            TokenKind::Sync => {
                marker = match (marker.is_async(), star) {
                    (true, true) => AsyncMarker::AsyncStar,
                    (true, false) => AsyncMarker::Async,
                    (false, true) => AsyncMarker::SyncStar,
                    (false, false) => AsyncMarker::Sync,
                }
            }
            _ => {}
        }
    }
    marker
}
fn top_span(node: SyntaxNode<'_>) -> Span {
    let start = node
        .children_with_tokens()
        .find(|element| match element {
            SyntaxElement::Node(node) => node.kind() != NodeKind::Annotation,
            SyntaxElement::Token(token) => !token.is_trivia(),
        })
        .map_or(node.span().start, |element| element.span().start);
    Span::new(start, node.span().end)
}
macro_rules! view { ($name:ident { $($field:ident),* $(,)? }) => { typed_node!($name, $name); debug_node!($name { $($field),* }); }; }
macro_rules! annotations { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn annotations(self) -> AstChildren<'a, Annotation<'a>> { AstChildren::new(self.0) } })* }; }
macro_rules! named { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn name(self) -> Identifier<'a> { name(self.0) } })* }; }
macro_rules! types { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> { AstChildren::new(self.0) } })* }; }
macro_rules! members { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn members(self) -> AstChildren<'a, ClassMember<'a>> { AstChildren::new(self.0) } })* }; }
macro_rules! flags { ($method:ident, $kind:ident; $($name:ident),* $(,)?) => { $(impl $name<'_> { pub fn $method(self) -> bool { has(self.0, TokenKind::$kind) } })* }; }
macro_rules! bodies { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn body(self) -> Option<FunctionBody<'a>> { child(self.0) } })* }; }
macro_rules! return_types { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn return_type(self) -> Option<DartType<'a>> { child(self.0) } })* }; }
macro_rules! params { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn params(self) -> FormalParamList<'a> { required(self.0) } })* }; }
macro_rules! async_markers { ($($name:ident),* $(,)?) => { $(impl $name<'_> { pub fn async_marker(self) -> AsyncMarker { asynchronous(self.0) } })* }; }
macro_rules! implements { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn implements(self) -> AstChildren<'a, DartType<'a>> { clause(self.0, NodeKind::ImplementsClause) } })* }; }
macro_rules! with_clause { ($($name:ident),* $(,)?) => { $(impl<'a> $name<'a> { pub fn with_clause(self) -> AstChildren<'a, DartType<'a>> { clause(self.0, NodeKind::WithClause) } })* }; }
macro_rules! top_view { ($name:ident { $($field:ident),* $(,)? }) => {
    #[derive(Clone, Copy)] pub struct $name<'a>(pub(crate) SyntaxNode<'a>);
    impl<'a> AstNode<'a> for $name<'a> { fn cast(node: SyntaxNode<'a>) -> Option<Self> { (node.kind() == NodeKind::$name).then_some(Self(node)) } fn syntax(self) -> SyntaxNode<'a> { self.0 } fn span(self) -> Span { top_span(self.0) } }
    impl<'a> $name<'a> { pub fn cast(node: SyntaxNode<'a>) -> Option<Self> { <Self as AstNode>::cast(node) } pub fn syntax(self) -> SyntaxNode<'a> { self.0 } pub fn span(self) -> Span { top_span(self.0) } }
    debug_node!($name { $($field),* });
}; }

#[derive(Clone, Copy)]
pub struct Program<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for Program<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::Program).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> Program<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        Span::new(
            0,
            self.0
                .significant_tokens()
                .next_back()
                .map_or(0, |t| t.span().end),
        )
    }
    pub fn library_directive(self) -> Option<LibraryDirective<'a>> {
        child(self.0)
    }
    pub fn part_of_directive(self) -> Option<PartOfDirective<'a>> {
        child(self.0)
    }
    pub fn part_directives(self) -> AstChildren<'a, PartDirective<'a>> {
        AstChildren::new(self.0)
    }
    pub fn imports(self) -> AstChildren<'a, ImportDirective<'a>> {
        AstChildren::new(self.0)
    }
    pub fn exports(self) -> AstChildren<'a, ExportDirective<'a>> {
        AstChildren::new(self.0)
    }
    pub fn declarations(self) -> AstChildren<'a, TopLevelDecl<'a>> {
        AstChildren::new(self.0)
    }
}
debug_node!(Program {
    library_directive,
    part_of_directive,
    part_directives,
    imports,
    exports,
    declarations,
    span
});

typed_node!(LibraryDirective, LibraryDirective);
typed_node!(PartOfDirective, PartOfDirective);
impl<'a> LibraryDirective<'a> {
    pub fn name(self) -> impl DoubleEndedIterator<Item = Identifier<'a>> + Clone {
        names(self.0)
    }
}
impl<'a> PartOfDirective<'a> {
    pub fn name(self) -> impl DoubleEndedIterator<Item = Identifier<'a>> + Clone {
        names(self.0)
    }
    pub fn uri(self) -> Option<StringLitNode<'a>> {
        child(self.0)
    }
}
impl std::fmt::Debug for LibraryDirective<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryDirective")
            .field("annotations", &self.annotations())
            .field("name", &DebugList(self.name()))
            .field("span", &self.span())
            .finish()
    }
}
impl std::fmt::Debug for PartOfDirective<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartOfDirective")
            .field("annotations", &self.annotations())
            .field("uri", &self.uri())
            .field("name", &DebugList(self.name()))
            .field("span", &self.span())
            .finish()
    }
}
view!(PartDirective {
    annotations,
    uri,
    span
});
view!(ImportDirective {
    annotations,
    uri,
    configurable_uris,
    is_deferred,
    as_name,
    combinators,
    span
});
view!(ExportDirective {
    annotations,
    uri,
    configurable_uris,
    combinators,
    span
});
impl<'a> PartDirective<'a> {
    pub fn uri(self) -> StringLitNode<'a> {
        required(self.0)
    }
}
macro_rules! uri_directives { ($($name:ident),*) => { $(impl<'a> $name<'a> {
    pub fn uri(self) -> StringLitNode<'a> { required(self.0) }
    pub fn configurable_uris(self) -> AstChildren<'a, ConfigurableUri<'a>> { AstChildren::new(self.0) }
    pub fn combinators(self) -> AstChildren<'a, ImportCombinator<'a>> { AstChildren::new(self.0) }
})* }; }
uri_directives!(ImportDirective, ExportDirective);
impl<'a> ImportDirective<'a> {
    pub fn as_name(self) -> Option<Identifier<'a>> {
        names(self.0).next()
    }
}
flags!(is_deferred, Deferred; ImportDirective);
typed_node!(ConfigurableUri, ConfigurableUri);
impl<'a> ConfigurableUri<'a> {
    pub fn test(self) -> impl DoubleEndedIterator<Item = Identifier<'a>> + Clone {
        names(self.0)
    }
    pub fn value(self) -> Option<StringLitNode<'a>> {
        let mut strings = AstChildren::<StringLitNode>::new(self.0);
        (strings.len() > 1).then(|| strings.next().expect("comparison literal"))
    }
    pub fn uri(self) -> StringLitNode<'a> {
        AstChildren::<StringLitNode>::new(self.0)
            .last()
            .expect("parser emits configurable URI")
    }
}
impl std::fmt::Debug for ConfigurableUri<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigurableUri")
            .field("test", &DebugList(self.test()))
            .field("value", &self.value())
            .field("uri", &self.uri())
            .field("span", &self.span())
            .finish()
    }
}
typed_node!(Combinator, Combinator);
impl<'a> Combinator<'a> {
    pub fn names(self) -> impl DoubleEndedIterator<Item = Identifier<'a>> + Clone {
        names(self.0)
    }
}
impl std::fmt::Debug for Combinator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ImportCombinator::cast(self.0)
            .expect("combinator kind")
            .fmt(f)
    }
}
#[derive(Clone, Copy)]
pub enum ImportCombinator<'a> {
    Show(Combinator<'a>),
    Hide(Combinator<'a>),
}
impl<'a> AstNode<'a> for ImportCombinator<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        let combinator = Combinator::cast(node)?;
        Some(if has(node, TokenKind::Show) {
            Self::Show(combinator)
        } else {
            Self::Hide(combinator)
        })
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::Show(c) | Self::Hide(c) => c.syntax(),
        }
    }
}
impl<'a> ImportCombinator<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        self.syntax().span()
    }
}
impl std::fmt::Debug for ImportCombinator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (variant, value) = match *self {
            Self::Show(v) => ("Show", v),
            Self::Hide(v) => ("Hide", v),
        };
        f.debug_tuple(variant)
            .field(&DebugList(value.names()))
            .field(&value.span())
            .finish()
    }
}

macro_rules! view_enum { ($name:ident { $($variant:ident($inner:ident)),* $(,)? }) => {
    #[derive(Clone, Copy)] pub enum $name<'a> { $($variant($inner<'a>)),* }
    impl<'a> AstNode<'a> for $name<'a> {
        fn cast(node: SyntaxNode<'a>) -> Option<Self> { $(if let Some(value) = $inner::cast(node) { return Some(Self::$variant(value)); })* None }
        fn syntax(self) -> SyntaxNode<'a> { match self { $(Self::$variant(value) => value.syntax()),* } }
        fn span(self) -> Span { match self { $(Self::$variant(value) => value.span()),* } }
    }
    impl<'a> $name<'a> { pub fn cast(node: SyntaxNode<'a>) -> Option<Self> { <Self as AstNode>::cast(node) } pub fn syntax(self) -> SyntaxNode<'a> { <Self as AstNode>::syntax(self) } pub fn span(self) -> Span { <Self as AstNode>::span(self) } }
    impl std::fmt::Debug for $name<'_> { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { $(Self::$variant(value) => f.debug_tuple(stringify!($variant)).field(value).finish()),* } } }
}; }
view_enum!(TopLevelDecl { Class(ClassDecl), ClassTypeAlias(ClassTypeAliasDecl), Mixin(MixinDecl), MixinClass(MixinClassDecl), Enum(EnumDecl), Extension(ExtensionDecl), ExtensionType(ExtensionTypeDecl), Function(FunctionDecl), Variable(TopLevelVarDecl), TypeAlias(TypeAliasDecl), Error(ErrorNode) });
view_enum!(ClassMember { Field(FieldDecl), Constructor(ConstructorDecl), Method(MethodDecl), Getter(GetterDecl), Setter(SetterDecl), Operator(OperatorDecl), PrimaryConstructorBody(PrimaryConstructorBody), Error(ErrorNode) });
top_view!(ClassDecl {
    annotations,
    modifiers,
    name,
    type_params,
    primary_constructor,
    extends,
    with_clause,
    implements,
    members,
    span
});
top_view!(ClassTypeAliasDecl {
    annotations,
    modifiers,
    name,
    type_params,
    superclass,
    with_clause,
    implements,
    span
});
top_view!(MixinDecl {
    annotations,
    is_base,
    name,
    type_params,
    on_clause,
    implements,
    members,
    span
});
top_view!(MixinClassDecl {
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
    span
});
top_view!(EnumDecl {
    annotations,
    name,
    type_params,
    primary_constructor,
    with_clause,
    implements,
    variants,
    members,
    span
});
top_view!(ExtensionDecl {
    annotations,
    name,
    type_params,
    on_type,
    members,
    span
});
top_view!(ExtensionTypeDecl {
    annotations,
    name,
    type_params,
    primary_constructor,
    implements,
    members,
    span
});
macro_rules! modifiers { ($($name:ident),*) => { $(impl $name<'_> { pub fn modifiers(self) -> ClassModifiers { ClassModifiers { is_abstract: has(self.0, TokenKind::Abstract), restriction: restriction(self.0) } } })* }; }
modifiers!(ClassDecl, ClassTypeAliasDecl);
macro_rules! optional_primary { ($($name:ident),*) => { $(impl<'a> $name<'a> { pub fn primary_constructor(self) -> Option<PrimaryConstructor<'a>> { child(self.0) } })* }; }
optional_primary!(ClassDecl, MixinClassDecl, EnumDecl);
impl<'a> ExtensionTypeDecl<'a> {
    pub fn primary_constructor(self) -> PrimaryConstructor<'a> {
        required(self.0)
    }
    pub fn representation(self) -> Option<FormalParam<'a>> {
        self.primary_constructor().params().params().first()
    }
}
macro_rules! extends { ($($name:ident),*) => { $(impl<'a> $name<'a> { pub fn extends(self) -> Option<DartType<'a>> { clause(self.0, NodeKind::ExtendsClause).next() } })* }; }
extends!(ClassDecl, MixinClassDecl);
impl<'a> ClassTypeAliasDecl<'a> {
    pub fn superclass(self) -> DartType<'a> {
        clause(self.0, NodeKind::ExtendsClause)
            .next()
            .expect("class alias superclass")
    }
}
impl<'a> MixinDecl<'a> {
    pub fn on_clause(self) -> AstChildren<'a, DartType<'a>> {
        clause(self.0, NodeKind::OnClause)
    }
}
impl<'a> ExtensionDecl<'a> {
    pub fn name(self) -> Option<Identifier<'a>> {
        names(self.0).next()
    }
    pub fn on_type(self) -> DartType<'a> {
        clause(self.0, NodeKind::OnClause)
            .next()
            .expect("extension on type")
    }
}
impl<'a> EnumDecl<'a> {
    pub fn variants(self) -> AstChildren<'a, EnumVariant<'a>> {
        AstChildren::new(self.0)
    }
}
view!(EnumVariant {
    annotations,
    name,
    type_args,
    constructor_name,
    args,
    span
});
impl<'a> EnumVariant<'a> {
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        names(self.0).nth(1)
    }
    pub fn args(self) -> Option<ArgList<'a>> {
        child(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct PrimaryConstructor<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for PrimaryConstructor<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::PrimaryConstructor).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> PrimaryConstructor<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn name_span(self) -> Span {
        name(
            self.0
                .parent()
                .expect("primary constructor has declaration parent"),
        )
        .span()
    }
    pub fn is_const(self) -> bool {
        has(
            self.0.parent().expect("primary constructor parent"),
            TokenKind::Const,
        )
    }
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        names(self.0).find(|name| !name.is_missing())
    }
    pub fn span(self) -> Span {
        if let Some(missing) = names(self.0).find(|name| name.is_missing()) {
            return missing.span();
        }
        let parent = self.0.parent().expect("primary constructor parent");
        Span::new(
            parent
                .child_token(TokenKind::Const)
                .map_or(self.name_span().start, |token| token.span().start),
            self.0.span().end,
        )
    }
}
params!(PrimaryConstructor);
debug_node!(PrimaryConstructor {
    is_const,
    name_span,
    constructor_name,
    params,
    span
});
view!(PrimaryConstructorBody {
    annotations,
    this_span,
    initializers,
    body,
    span
});
impl PrimaryConstructorBody<'_> {
    pub fn this_span(self) -> Span {
        self.0
            .child_token(TokenKind::This)
            .expect("primary body this token")
            .span()
    }
}
view!(FieldDecl {
    annotations,
    is_static,
    is_abstract,
    is_external,
    is_covariant,
    is_late,
    keyword,
    field_type,
    declarators,
    span
});
view!(VarDeclarator {
    name,
    initializer,
    span
});
impl<'a> VarDeclarator<'a> {
    pub fn initializer(self) -> Option<Expr<'a>> {
        assigned_value(self.0)
    }
}
impl<'a> FieldDecl<'a> {
    pub fn keyword(self) -> VarKeyword {
        keyword(self.0)
    }
    pub fn field_type(self) -> Option<DartType<'a>> {
        child(self.0)
    }
    pub fn declarators(self) -> AstChildren<'a, VarDeclarator<'a>> {
        AstChildren::new(self.0)
    }
}
view!(ConstructorDecl {
    annotations,
    is_const,
    is_factory,
    is_external,
    name,
    constructor_name,
    params,
    initializers,
    redirect,
    body,
    span
});
impl<'a> ConstructorDecl<'a> {
    fn implicit_name(self) -> Option<Identifier<'a>> {
        let keyword = self
            .0
            .child_token(TokenKind::New)
            .or_else(|| self.0.child_token(TokenKind::Factory))?;
        let enclosing = self
            .0
            .ancestors()
            .skip(1)
            .find_map(|node| match node.kind() {
                NodeKind::ClassDecl
                | NodeKind::MixinClassDecl
                | NodeKind::EnumDecl
                | NodeKind::ExtensionTypeDecl => Some(name(node)),
                _ => None,
            })?;
        let explicit = has(self.0, TokenKind::Dot)
            || names(self.0)
                .next()
                .is_some_and(|n| n.is_missing() || n.name() == enclosing.name());
        (!explicit).then_some(enclosing.with_span(keyword.span()))
    }
    pub fn name(self) -> Identifier<'a> {
        self.implicit_name().unwrap_or_else(|| name(self.0))
    }
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        names(self.0).nth(usize::from(self.implicit_name().is_none()))
    }
    pub fn redirect(self) -> Option<RedirectedConstructor<'a>> {
        child(self.0)
    }
}
view!(RedirectedConstructor {
    type_,
    constructor_name,
    span
});
impl<'a> RedirectedConstructor<'a> {
    pub fn type_(self) -> DartType<'a> {
        required(self.0)
    }
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        names(self.0).next()
    }
}
macro_rules! initializers { ($($name:ident),*) => { $(impl<'a> $name<'a> { pub fn initializers(self) -> AstChildren<'a, ConstructorInitializer<'a>> { AstChildren::new(self.0) } })* }; }
initializers!(ConstructorDecl, PrimaryConstructorBody);
typed_node!(SuperCallInitializer, SuperCallInitializer);
typed_node!(ThisCallInitializer, ThisCallInitializer);
typed_node!(FieldInitializer, FieldInitializer);
typed_node!(AssertInitializer, AssertInitializer);
macro_rules! call_init { ($($name:ident),*) => { $(impl<'a> $name<'a> { pub fn call_name(self) -> Option<Identifier<'a>> { names(self.0).next() } pub fn args(self) -> ArgList<'a> { required(self.0) } })* }; }
call_init!(SuperCallInitializer, ThisCallInitializer);
impl<'a> FieldInitializer<'a> {
    pub fn field(self) -> Identifier<'a> {
        name(self.0)
    }
    pub fn value(self) -> Expr<'a> {
        AstChildren::<Expr>::new(self.0)
            .next_back()
            .expect("field initializer value")
    }
}
impl<'a> AssertInitializer<'a> {
    pub fn condition(self) -> Expr<'a> {
        required(self.0)
    }
    pub fn message(self) -> Option<Expr<'a>> {
        AstChildren::<Expr>::new(self.0).nth(1)
    }
}
debug_node!(SuperCallInitializer => "SuperCall" { call_name, args, span });
debug_node!(ThisCallInitializer => "ThisCall" { call_name, args, span });
debug_node!(FieldInitializer => "FieldInit" { field, value, span });
debug_node!(AssertInitializer => "Assert" { condition, message, span });
#[derive(Clone, Copy)]
pub enum ConstructorInitializer<'a> {
    SuperCall(SuperCallInitializer<'a>),
    ThisCall(ThisCallInitializer<'a>),
    FieldInit(FieldInitializer<'a>),
    Assert(AssertInitializer<'a>),
}
impl<'a> AstNode<'a> for ConstructorInitializer<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        match node.kind() {
            NodeKind::SuperCallInitializer => Some(Self::SuperCall(SuperCallInitializer(node))),
            NodeKind::ThisCallInitializer => Some(Self::ThisCall(ThisCallInitializer(node))),
            NodeKind::FieldInitializer => Some(Self::FieldInit(FieldInitializer(node))),
            NodeKind::AssertInitializer => Some(Self::Assert(AssertInitializer(node))),
            _ => None,
        }
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::SuperCall(v) => v.syntax(),
            Self::ThisCall(v) => v.syntax(),
            Self::FieldInit(v) => v.syntax(),
            Self::Assert(v) => v.syntax(),
        }
    }
}
impl<'a> ConstructorInitializer<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        self.syntax().span()
    }
}
impl std::fmt::Debug for ConstructorInitializer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SuperCall(v) => v.fmt(f),
            Self::ThisCall(v) => v.fmt(f),
            Self::FieldInit(v) => v.fmt(f),
            Self::Assert(v) => v.fmt(f),
        }
    }
}

view!(MethodDecl {
    annotations,
    is_static,
    is_abstract,
    is_external,
    async_marker,
    return_type,
    name,
    type_params,
    params,
    body,
    span
});
view!(GetterDecl {
    annotations,
    is_static,
    is_abstract,
    is_external,
    async_marker,
    return_type,
    name,
    body,
    span
});
view!(SetterDecl {
    annotations,
    is_static,
    is_abstract,
    is_external,
    is_covariant,
    param_type,
    param_annotations,
    name,
    param,
    body,
    span
});
view!(OperatorDecl {
    annotations,
    is_external,
    return_type,
    op,
    params,
    async_marker,
    body,
    span
});
impl<'a> SetterDecl<'a> {
    fn parameter(self) -> SyntaxNode<'a> {
        self.0
            .children()
            .find(|n| n.kind() == NodeKind::SetterParameter)
            .expect("setter parameter role")
    }
    pub fn param_type(self) -> Option<DartType<'a>> {
        let p = self.parameter();
        if child::<FormalParamList>(p).is_some() {
            Some(DartType::Function(
                FunctionType::cast(p).expect("setter function type"),
            ))
        } else {
            child(p)
        }
    }
    pub fn param_annotations(self) -> AstChildren<'a, Annotation<'a>> {
        AstChildren::new(self.parameter())
    }
    pub fn is_covariant(self) -> bool {
        has(self.parameter(), TokenKind::Covariant)
    }
    pub fn param(self) -> Identifier<'a> {
        name(self.parameter())
    }
}
impl<'a> OperatorDecl<'a> {
    pub fn op(self) -> &'a str {
        let token = self
            .0
            .child_token(TokenKind::Operator)
            .expect("operator keyword")
            .next_significant()
            .expect("operator symbol");
        if token.kind() != TokenKind::LBracket {
            return token.text();
        }
        let next = token.next_significant();
        let after_close = next.and_then(|token| {
            if token.kind() == TokenKind::RBracket {
                token.next_significant()
            } else {
                Some(token)
            }
        });
        if after_close.is_some_and(|token| token.kind() == TokenKind::Eq) {
            "[]="
        } else {
            "[]"
        }
    }
    pub fn lookup_name(self) -> &'a str {
        if self.op() == "-" && self.params().params().is_empty() {
            "unary-"
        } else {
            self.op()
        }
    }
}
impl MethodDecl<'_> {
    pub fn is_abstract(self) -> bool {
        has(self.0, TokenKind::Abstract) || self.body().is_none()
    }
}
macro_rules! abstract_accessors { ($($name:ident),*) => { $(impl $name<'_> { pub fn is_abstract(self) -> bool { has(self.0, TokenKind::Abstract) || (self.body().is_none() && !self.is_external()) } })* }; }
abstract_accessors!(GetterDecl, SetterDecl);
top_view!(FunctionDecl {
    annotations,
    is_external,
    async_marker,
    kind,
    return_type,
    name,
    type_params,
    body,
    span
});
#[derive(Clone, Copy, Debug)]
pub enum FunctionKind<'a> {
    Function(FormalParamList<'a>),
    Getter,
    Setter(FormalParamList<'a>),
}
impl<'a> FunctionDecl<'a> {
    pub fn kind(self) -> FunctionKind<'a> {
        if has(self.0, TokenKind::Get) {
            FunctionKind::Getter
        } else if has(self.0, TokenKind::Set) {
            FunctionKind::Setter(required(self.0))
        } else {
            FunctionKind::Function(required(self.0))
        }
    }
    pub fn params(self) -> Option<FormalParamList<'a>> {
        match self.kind() {
            FunctionKind::Getter => None,
            FunctionKind::Function(p) | FunctionKind::Setter(p) => Some(p),
        }
    }
}
top_view!(TopLevelVarDecl {
    annotations,
    is_external,
    keyword,
    is_late,
    var_type,
    declarators,
    span
});
impl<'a> TopLevelVarDecl<'a> {
    pub fn keyword(self) -> VarKeyword {
        if has(self.0, TokenKind::Const) {
            VarKeyword::Const
        } else if restriction(self.0) == Some(ClassRestriction::Final) {
            VarKeyword::Final
        } else {
            VarKeyword::Var
        }
    }
    pub fn var_type(self) -> Option<DartType<'a>> {
        child(self.0)
    }
    pub fn declarators(self) -> AstChildren<'a, VarDeclarator<'a>> {
        AstChildren::new(self.0)
    }
}
top_view!(TypeAliasDecl {
    annotations,
    name,
    type_params,
    aliased,
    legacy_params,
    span
});
impl<'a> TypeAliasDecl<'a> {
    fn legacy(self) -> Option<SyntaxNode<'a>> {
        self.0
            .children()
            .find(|n| n.kind() == NodeKind::LegacyTypedef)
    }
    pub fn name(self) -> Identifier<'a> {
        if let Some(legacy) = self.legacy() {
            names(legacy).next().unwrap_or_else(|| {
                let leading = required::<DartType>(legacy);
                if let DartType::Named(named) = leading
                    && let Some(name) = named.segments().next_back()
                {
                    return name;
                }
                Identifier::synthetic(leading.syntax(), "<error>", leading.span())
            })
        } else {
            name(self.0)
        }
    }
    pub fn type_params(self) -> AstChildren<'a, TypeParam<'a>> {
        let parent = self.legacy().unwrap_or(self.0);
        if self.legacy().is_some()
            && names(parent).next().is_none()
            && let Some(args) = parent
                .children()
                .find(|n| n.kind() == NodeKind::NamedType)
                .and_then(|n| n.children().find(|n| n.kind() == NodeKind::TypeArgs))
        {
            return AstChildren::new(args);
        }
        AstChildren::new(parent)
    }
    pub fn aliased(self) -> DartType<'a> {
        if let Some(legacy) = self.legacy() {
            DartType::Function(FunctionType::cast(legacy).expect("legacy function type"))
        } else {
            required(self.0)
        }
    }
    pub fn legacy_params(self) -> Option<FormalParamList<'a>> {
        self.legacy().and_then(child)
    }
}
#[derive(Clone, Copy)]
pub struct FormalParamList<'a>(pub(crate) SyntaxNode<'a>);
impl<'a> AstNode<'a> for FormalParamList<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        (node.kind() == NodeKind::FormalParamList).then_some(Self(node))
    }
    fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    fn span(self) -> Span {
        self.span()
    }
}
debug_node!(FormalParamList { params, span });
impl<'a> FormalParamList<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        self.0
    }
    pub fn span(self) -> Span {
        names(self.0)
            .find(|name| name.is_missing())
            .map_or(self.0.span(), |name| name.span())
    }
    pub fn params(self) -> AstChildren<'a, FormalParam<'a>> {
        AstChildren::new(self.0)
    }
    pub fn positional(self) -> impl Iterator<Item = FormalParam<'a>> + Clone {
        self.params()
            .filter(|p| !matches!(p.kind(), ParamKind::Named { .. }))
    }
    pub fn named(self) -> impl Iterator<Item = FormalParam<'a>> + Clone {
        self.params()
            .filter(|p| matches!(p.kind(), ParamKind::Named { .. }))
    }
}
view!(FormalParam {
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
    span
});
impl<'a> FormalParam<'a> {
    pub fn kind(self) -> ParamKind {
        self.0.param_kind()
    }
    pub fn initializing(self) -> Option<InitializingFormal> {
        if has(self.0, TokenKind::This) {
            Some(InitializingFormal::This)
        } else if has(self.0, TokenKind::Super) {
            Some(InitializingFormal::Super)
        } else {
            None
        }
    }
    pub fn param_type(self) -> Option<DartType<'a>> {
        child(self.0)
    }
    pub fn default_value(self) -> Option<Expr<'a>> {
        assigned_value(self.0)
    }
    pub fn function_params(self) -> Option<FormalParamList<'a>> {
        child(self.0)
    }
}
typed_node!(ArrowBody, ArrowBody);
impl<'a> ArrowBody<'a> {
    pub fn expr(self) -> Expr<'a> {
        required(self.0)
    }
}
typed_node!(NativeBody, NativeBody);
impl<'a> NativeBody<'a> {
    pub fn literal(self) -> Option<StringLitNode<'a>> {
        child(self.0)
    }
}
debug_tuple!(ArrowBody => "Arrow" { expr, span });
debug_tuple!(NativeBody => "Native" { literal, span });
#[derive(Clone, Copy)]
pub enum FunctionBody<'a> {
    Block(Block<'a>),
    Arrow(ArrowBody<'a>),
    Native(NativeBody<'a>),
}
impl<'a> AstNode<'a> for FunctionBody<'a> {
    fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        if let Some(v) = Block::cast(node) {
            Some(Self::Block(v))
        } else if let Some(v) = ArrowBody::cast(node) {
            Some(Self::Arrow(v))
        } else {
            NativeBody::cast(node).map(Self::Native)
        }
    }
    fn syntax(self) -> SyntaxNode<'a> {
        match self {
            Self::Block(v) => v.syntax(),
            Self::Arrow(v) => v.syntax(),
            Self::Native(v) => v.syntax(),
        }
    }
    fn span(self) -> Span {
        self.span()
    }
}
impl<'a> FunctionBody<'a> {
    pub fn cast(node: SyntaxNode<'a>) -> Option<Self> {
        <Self as AstNode>::cast(node)
    }
    pub fn syntax(self) -> SyntaxNode<'a> {
        <Self as AstNode>::syntax(self)
    }
    pub fn span(self) -> Span {
        match self {
            Self::Block(body) => body.span(),
            Self::Arrow(body) => body.span(),
            Self::Native(body) => body.span(),
        }
    }
}
impl std::fmt::Debug for FunctionBody<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Block(v) => f.debug_tuple("Block").field(v).finish(),
            Self::Arrow(v) => f
                .debug_tuple("Arrow")
                .field(&v.expr())
                .field(&v.span())
                .finish(),
            Self::Native(v) => f
                .debug_tuple("Native")
                .field(&v.literal())
                .field(&v.span())
                .finish(),
        }
    }
}
typed_node!(Annotation, Annotation);
impl<'a> Annotation<'a> {
    pub fn name(self) -> impl DoubleEndedIterator<Item = Identifier<'a>> + Clone {
        let limit = self
            .0
            .children()
            .find(|n| n.kind() == NodeKind::TypeArgs)
            .map(|n| n.span().start);
        names(self.0).filter(move |n| limit.is_none_or(|end| n.span().start < end))
    }
    pub fn type_args(self) -> AstChildren<'a, DartType<'a>> {
        TypeArgs::of(self.0)
    }
    pub fn constructor_name(self) -> Option<Identifier<'a>> {
        let end = self
            .0
            .children()
            .find(|n| n.kind() == NodeKind::TypeArgs)?
            .span()
            .end;
        names(self.0).find(|n| n.span().start >= end)
    }
    pub fn args(self) -> Option<ArgList<'a>> {
        child(self.0)
    }
}
impl std::fmt::Debug for Annotation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Annotation")
            .field("name", &DebugList(self.name()))
            .field("type_args", &self.type_args())
            .field("constructor_name", &self.constructor_name())
            .field("args", &self.args())
            .field("span", &self.span())
            .finish()
    }
}

annotations!(
    LibraryDirective,
    PartOfDirective,
    PartDirective,
    ImportDirective,
    ExportDirective,
    ClassDecl,
    ClassTypeAliasDecl,
    MixinDecl,
    MixinClassDecl,
    EnumDecl,
    EnumVariant,
    ExtensionDecl,
    ExtensionTypeDecl,
    PrimaryConstructorBody,
    FieldDecl,
    ConstructorDecl,
    MethodDecl,
    GetterDecl,
    SetterDecl,
    OperatorDecl,
    FunctionDecl,
    TopLevelVarDecl,
    TypeAliasDecl,
    FormalParam
);
named!(
    ClassDecl,
    ClassTypeAliasDecl,
    MixinDecl,
    MixinClassDecl,
    EnumDecl,
    EnumVariant,
    ExtensionTypeDecl,
    VarDeclarator,
    MethodDecl,
    GetterDecl,
    SetterDecl,
    FunctionDecl,
    FormalParam
);
types!(
    ClassDecl,
    ClassTypeAliasDecl,
    MixinDecl,
    MixinClassDecl,
    EnumDecl,
    ExtensionDecl,
    ExtensionTypeDecl,
    MethodDecl,
    FunctionDecl,
    FormalParam
);
members!(
    ClassDecl,
    MixinDecl,
    MixinClassDecl,
    EnumDecl,
    ExtensionDecl,
    ExtensionTypeDecl
);
implements!(
    ClassDecl,
    ClassTypeAliasDecl,
    MixinDecl,
    MixinClassDecl,
    EnumDecl,
    ExtensionTypeDecl
);
with_clause!(ClassDecl, ClassTypeAliasDecl, MixinClassDecl, EnumDecl);
impl MixinDecl<'_> {
    pub fn is_base(self) -> bool {
        restriction(self.0) == Some(ClassRestriction::Base)
    }
}
impl MixinClassDecl<'_> {
    pub fn is_base(self) -> bool {
        restriction(self.0) == Some(ClassRestriction::Base)
    }
}
flags!(is_abstract, Abstract; MixinClassDecl, FieldDecl);
flags!(is_static, Static; FieldDecl, MethodDecl, GetterDecl, SetterDecl);
flags!(is_external, External; FieldDecl, ConstructorDecl, MethodDecl, GetterDecl, SetterDecl, OperatorDecl, FunctionDecl, TopLevelVarDecl);
flags!(is_covariant, Covariant; FieldDecl, FormalParam);
flags!(is_late, Late; FieldDecl, TopLevelVarDecl);
flags!(is_const, Const; ConstructorDecl);
flags!(is_factory, Factory; ConstructorDecl);
flags!(is_final, Final; FormalParam);
flags!(is_var, Var; FormalParam);
bodies!(
    PrimaryConstructorBody,
    ConstructorDecl,
    MethodDecl,
    GetterDecl,
    SetterDecl,
    OperatorDecl,
    FunctionDecl
);
return_types!(MethodDecl, GetterDecl, OperatorDecl, FunctionDecl);
params!(ConstructorDecl, MethodDecl, OperatorDecl);
async_markers!(MethodDecl, GetterDecl, OperatorDecl, FunctionDecl);

#[derive(Clone)]
pub struct EffectiveMembers<'a> {
    pub declared: AstChildren<'a, ClassMember<'a>>,
    pub primary: Option<PrimaryConstructor<'a>>,
    pub representation: bool,
}
impl<'a> EffectiveMembers<'a> {
    pub fn declared(members: AstChildren<'a, ClassMember<'a>>) -> Self {
        Self {
            declared: members,
            primary: None,
            representation: false,
        }
    }
    pub fn declaring_params(&self) -> impl Iterator<Item = FormalParam<'a>> {
        let representation = self.representation;
        self.primary
            .into_iter()
            .flat_map(|p| p.params().params())
            .filter(move |p| representation || p.is_var() || p.is_final())
    }
    pub fn constructors(&self) -> impl Iterator<Item = ConstructorRef<'a>> {
        let body = self.primary.and_then(|_| {
            self.declared.clone().find_map(|m| {
                if let ClassMember::PrimaryConstructorBody(body) = m {
                    Some(body)
                } else {
                    None
                }
            })
        });
        self.declared
            .clone()
            .filter_map(|member| {
                if let ClassMember::Constructor(c) = member {
                    Some(ConstructorRef {
                        annotations: c.annotations(),
                        is_const: c.is_const(),
                        is_factory: c.is_factory(),
                        is_external: c.is_external(),
                        is_primary: false,
                        constructor_name: c.constructor_name(),
                        params: c.params(),
                        initializers: c.initializers(),
                        redirect: c.redirect(),
                        body: c.body(),
                        name_span: c.name().span(),
                        span: c.span(),
                    })
                } else {
                    None
                }
            })
            .chain(self.primary.map(|p| ConstructorRef {
                annotations: AstChildren::new(body.map_or(p.syntax(), |b| b.syntax())),
                is_const: p.is_const(),
                is_factory: false,
                is_external: false,
                is_primary: true,
                constructor_name: p.constructor_name(),
                params: p.params(),
                initializers: AstChildren::new(body.map_or(p.syntax(), |b| b.syntax())),
                redirect: None,
                body: body.and_then(|b| b.body()),
                name_span: p.name_span(),
                span: p.span(),
            }))
    }
}
#[derive(Clone)]
pub struct ConstructorRef<'a> {
    pub annotations: AstChildren<'a, Annotation<'a>>,
    pub is_const: bool,
    pub is_factory: bool,
    pub is_external: bool,
    pub is_primary: bool,
    pub constructor_name: Option<Identifier<'a>>,
    pub params: FormalParamList<'a>,
    pub initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    pub redirect: Option<RedirectedConstructor<'a>>,
    pub body: Option<FunctionBody<'a>>,
    pub name_span: Span,
    pub span: Span,
}
macro_rules! elements { ($($name:ident),*) => { $(impl<'a> $name<'a> { pub fn element_members(self) -> EffectiveMembers<'a> { EffectiveMembers { declared: self.members(), primary: self.primary_constructor(), representation: false } } })* }; }
elements!(ClassDecl, MixinClassDecl, EnumDecl);
impl<'a> ExtensionTypeDecl<'a> {
    pub fn element_members(self) -> EffectiveMembers<'a> {
        EffectiveMembers {
            declared: self.members(),
            primary: Some(self.primary_constructor()),
            representation: true,
        }
    }
}
