/// Byte-range span for source attribution.
///
/// Parsed syntax uses half-open byte offsets ending at the last consumed token,
/// excluding following whitespace and comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// An identifier (name) with its source location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

impl Identifier {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

/// A raw string literal value with its source location.
#[derive(Debug, Clone)]
pub struct StringLitNode {
    /// Raw literal text including quotes, concatenated for adjacent literals.
    pub raw: String,
    /// Whether multiple source literals contributed to this node.
    pub is_adjacent: bool,
    /// Source spans of adjacent segments; empty for a single literal.
    pub segment_spans: Vec<Span>,
    /// Whether any contributing source literal uses a raw prefix.
    pub has_raw_segment: bool,
    /// The content without delimiters; escape sequences remain unresolved.
    pub value: String,
    pub span: Span,
    /// The parsed interpolated expressions (`$id` / `${expr}`) embedded in the
    /// literal, in source order. Empty for raw strings, non-interpolated
    /// literals, and any interpolation whose inner expression failed to parse
    /// cleanly (recorded conservatively as absent).
    pub interpolations: Vec<StringInterpolation>,
}

/// Delimiter and undecoded content of one string literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringDelimiter<'a> {
    pub is_raw: bool,
    pub quote: u8,
    pub triple: bool,
    pub content: &'a str,
    pub content_offset: usize,
    pub terminated: bool,
}

impl StringLitNode {
    pub fn segments(&self) -> impl Iterator<Item = (StringDelimiter<'_>, Span)> {
        let spans = if self.segment_spans.is_empty() {
            std::slice::from_ref(&self.span)
        } else {
            &self.segment_spans
        };
        let mut offset = 0;
        spans.iter().filter_map(move |&span| {
            let end = offset + span.end - span.start;
            let raw = self.raw.get(offset..end)?;
            offset = end;
            parse_string_delimiter(raw).map(|delimiter| (delimiter, span))
        })
    }

    /// Returns no delimiter for concatenated segments, which may use different quoting.
    pub fn delimiter(&self) -> Option<StringDelimiter<'_>> {
        if self.is_adjacent {
            return None;
        }
        parse_string_delimiter(&self.raw)
    }
}

/// Split one source string literal (`r`, quotes, content) into its parts.
pub fn parse_string_delimiter(raw: &str) -> Option<StringDelimiter<'_>> {
    let is_raw = raw.starts_with('r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let quote = *rest.as_bytes().first()?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let triple = rest.as_bytes().get(..3) == Some(&[quote; 3]);
    let len = if triple { 3 } else { 1 };
    let closing = &rest[..len];
    let terminated = rest.len() >= 2 * len && rest[len..].ends_with(closing);
    let content_offset = prefix + len;
    let content_end = if rest.ends_with(closing) {
        (raw.len() - len).max(content_offset)
    } else {
        raw.len()
    };
    Some(StringDelimiter {
        is_raw,
        quote,
        triple,
        content: &raw[content_offset..content_end],
        content_offset,
        terminated,
    })
}

/// A single interpolation embedded in a [`StringLitNode`].
#[derive(Debug, Clone)]
pub struct StringInterpolation {
    /// The parsed interpolated expression.
    pub expr: Expr,
    /// Absolute byte range of the interpolated expression text in the original
    /// source: for `${e}` the full interior of the braces, including trivia;
    /// for `$ident` the range of `ident`.
    pub span: Span,
}

/// A parsed Dart compilation unit (one `.dart` file).
#[derive(Debug, Clone)]
pub struct Program {
    pub library_directive: Option<LibraryDirective>,
    pub part_of_directive: Option<PartOfDirective>,
    pub part_directives: Vec<PartDirective>,
    pub imports: Vec<ImportDirective>,
    pub exports: Vec<ExportDirective>,
    pub declarations: Vec<TopLevelDecl>,
    pub span: Span,
}

/// `library foo.bar;`
#[derive(Debug, Clone)]
pub struct LibraryDirective {
    pub annotations: Vec<Annotation>,
    pub name: Vec<Identifier>,
    pub span: Span,
}

/// `part of 'uri';` or `part of foo.bar;`
#[derive(Debug, Clone)]
pub struct PartOfDirective {
    pub annotations: Vec<Annotation>,
    pub uri: Option<StringLitNode>,
    pub name: Vec<Identifier>,
    pub span: Span,
}

/// `part 'uri';`
#[derive(Debug, Clone)]
pub struct PartDirective {
    pub annotations: Vec<Annotation>,
    pub uri: StringLitNode,
    pub span: Span,
}

/// `import 'uri' [if (name) 'uri' ...] [as name] [show/hide ...];`
#[derive(Debug, Clone)]
pub struct ImportDirective {
    pub annotations: Vec<Annotation>,
    pub uri: StringLitNode,
    /// Configurable (conditional) URIs: `if (dart.library.io) 'io.dart'`.
    pub configurable_uris: Vec<ConfigurableUri>,
    pub is_deferred: bool,
    pub as_name: Option<Identifier>,
    pub combinators: Vec<ImportCombinator>,
    pub span: Span,
}

/// `export 'uri' [if (name) 'uri' ...] [show/hide ...];`
#[derive(Debug, Clone)]
pub struct ExportDirective {
    pub annotations: Vec<Annotation>,
    pub uri: StringLitNode,
    /// Configurable (conditional) URIs: `if (dart.library.io) 'io.dart'`.
    pub configurable_uris: Vec<ConfigurableUri>,
    pub combinators: Vec<ImportCombinator>,
    pub span: Span,
}

/// A configurable-URI clause on an import/export: `if (dotted.name [== 'value']) 'uri'`.
#[derive(Debug, Clone)]
pub struct ConfigurableUri {
    /// The dotted environment constant tested, e.g. `dart.library.io`.
    pub test: Vec<Identifier>,
    /// The `== 'value'` comparison string, when present.
    pub value: Option<StringLitNode>,
    /// The URI selected when the test succeeds.
    pub uri: StringLitNode,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImportCombinator {
    Show(Vec<Identifier>, Span),
    Hide(Vec<Identifier>, Span),
}

#[derive(Debug, Clone)]
pub enum TopLevelDecl {
    Class(ClassDecl),
    /// Mixin-application class: `class MA = S with M [implements I];`
    ClassTypeAlias(ClassTypeAliasDecl),
    Mixin(MixinDecl),
    MixinClass(MixinClassDecl),
    Enum(EnumDecl),
    Extension(ExtensionDecl),
    ExtensionType(ExtensionTypeDecl),
    Function(FunctionDecl),
    Variable(TopLevelVarDecl),
    TypeAlias(TypeAliasDecl),
    Error(ErrorNode),
}

impl TopLevelDecl {
    pub fn span(&self) -> &Span {
        match self {
            TopLevelDecl::Class(x) => &x.span,
            TopLevelDecl::ClassTypeAlias(x) => &x.span,
            TopLevelDecl::Mixin(x) => &x.span,
            TopLevelDecl::MixinClass(x) => &x.span,
            TopLevelDecl::Enum(x) => &x.span,
            TopLevelDecl::Extension(x) => &x.span,
            TopLevelDecl::ExtensionType(x) => &x.span,
            TopLevelDecl::Function(x) => &x.span,
            TopLevelDecl::Variable(x) => &x.span,
            TopLevelDecl::TypeAlias(x) => &x.span,
            TopLevelDecl::Error(x) => &x.span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassRestriction {
    Base,
    Interface,
    Final,
    Sealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClassModifiers {
    pub is_abstract: bool,
    pub restriction: Option<ClassRestriction>,
}

/// `[modifiers] class Name<T> [extends S] [with M] [implements I] { ... }`
#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub annotations: Vec<Annotation>,
    pub modifiers: ClassModifiers,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    /// Dart 3.13 primary constructor in the header: `class C(var int x)`.
    pub primary_constructor: Option<PrimaryConstructor>,
    pub extends: Option<DartType>,
    pub with_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

/// A primary constructor declared in a type header (the analyzer's
/// `PrimaryConstructorDeclaration`): `[const] Name<T>[.name](params)` in
/// `class`, `mixin class`, `enum` and `extension type` declarations.
///
/// Its `var`/`final` parameters ([`FormalParam::is_var`],
/// [`FormalParam::is_final`]) are *declaring*: each also declares an instance
/// field of the same name and type. An extension type's single representation
/// parameter always declares its representation field.
#[derive(Debug, Clone)]
pub struct PrimaryConstructor {
    pub is_const: bool,
    /// The declaring type's identifier, excluding `const` and type parameters.
    pub name_span: Span,
    /// `.name` of a named primary constructor: `class C.named(int x)`.
    pub constructor_name: Option<Identifier>,
    pub params: FormalParamList,
    /// From `const` (or the type name) through the closing `)`.
    pub span: Span,
}

/// Borrowed element-model inputs, without synthesizing or copying AST nodes.
#[derive(Clone, Copy)]
pub struct EffectiveMembers<'a> {
    pub declared: &'a [ClassMember],
    pub primary: Option<&'a PrimaryConstructor>,
    pub representation: bool,
}

impl<'a> EffectiveMembers<'a> {
    pub fn declared(members: &'a [ClassMember]) -> Self {
        Self {
            declared: members,
            primary: None,
            representation: false,
        }
    }

    /// Header parameters that also declare instance fields.
    pub fn declaring_params(self) -> impl Iterator<Item = &'a FormalParam> {
        self.primary
            .into_iter()
            .flat_map(|primary| &primary.params.params)
            .filter(move |param| self.representation || param.is_var || param.is_final)
    }

    pub fn constructors(self) -> impl Iterator<Item = ConstructorRef<'a>> {
        self.declared
            .iter()
            .filter_map(|member| match member {
                ClassMember::Constructor(constructor) => Some(ConstructorRef {
                    annotations: &constructor.annotations,
                    is_const: constructor.is_const,
                    is_factory: constructor.is_factory,
                    is_external: constructor.is_external,
                    is_primary: false,
                    constructor_name: constructor.constructor_name.as_ref(),
                    params: &constructor.params,
                    initializers: &constructor.initializers,
                    redirect: constructor.redirect.as_ref(),
                    body: constructor.body.as_ref(),
                    name_span: constructor.name.span,
                    span: constructor.span,
                }),
                _ => None,
            })
            .chain(self.primary.map(|primary| {
                let body = self.declared.iter().find_map(|member| match member {
                    ClassMember::PrimaryConstructorBody(body) => Some(body),
                    _ => None,
                });
                ConstructorRef {
                    annotations: body.map_or(&[][..], |body| body.annotations.as_slice()),
                    is_const: primary.is_const,
                    is_factory: false,
                    is_external: false,
                    is_primary: true,
                    constructor_name: primary.constructor_name.as_ref(),
                    params: &primary.params,
                    initializers: body.map_or(&[][..], |body| body.initializers.as_slice()),
                    redirect: None,
                    body: body.and_then(|body| body.body.as_ref()),
                    name_span: primary.name_span,
                    span: primary.span,
                }
            }))
    }
}

/// Shared constructor facts borrowed from ordinary or primary declarations.
#[derive(Clone, Copy)]
pub struct ConstructorRef<'a> {
    pub annotations: &'a [Annotation],
    pub is_const: bool,
    pub is_factory: bool,
    pub is_external: bool,
    pub is_primary: bool,
    pub constructor_name: Option<&'a Identifier>,
    pub params: &'a FormalParamList,
    pub initializers: &'a [ConstructorInitializer],
    pub redirect: Option<&'a RedirectedConstructor>,
    pub body: Option<&'a FunctionBody>,
    pub name_span: Span,
    pub span: Span,
}

impl ClassDecl {
    /// Declared members plus the constructor and fields of the primary
    /// constructor, if any — see [`PrimaryConstructor`].
    pub fn element_members(&self) -> EffectiveMembers<'_> {
        EffectiveMembers {
            primary: self.primary_constructor.as_ref(),
            declared: &self.members,
            representation: false,
        }
    }
}

/// The body part of a primary constructor, a class member starting with
/// `this` (the analyzer's `PrimaryConstructorBody`): `this : x = 1 { ... }`.
#[derive(Debug, Clone)]
pub struct PrimaryConstructorBody {
    pub annotations: Vec<Annotation>,
    /// The `this` keyword, excluding preceding annotations.
    pub this_span: Span,
    pub initializers: Vec<ConstructorInitializer>,
    /// `None` for a `;` body.
    pub body: Option<FunctionBody>,
    pub span: Span,
}

/// `[modifiers] class Name<T> = Superclass with M1, M2 [implements I];`
///
/// A mixin-application ("class type alias") declaration: names a class formed by
/// applying mixins to a superclass, with no body.
#[derive(Debug, Clone)]
pub struct ClassTypeAliasDecl {
    pub annotations: Vec<Annotation>,
    pub modifiers: ClassModifiers,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub superclass: DartType,
    pub with_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    Field(FieldDecl),
    Constructor(ConstructorDecl),
    Method(MethodDecl),
    Getter(GetterDecl),
    Setter(SetterDecl),
    Operator(OperatorDecl),
    PrimaryConstructorBody(PrimaryConstructorBody),
    Error(ErrorNode),
}

impl ClassMember {
    pub fn span(&self) -> &Span {
        match self {
            ClassMember::Field(x) => &x.span,
            ClassMember::Constructor(x) => &x.span,
            ClassMember::Method(x) => &x.span,
            ClassMember::Getter(x) => &x.span,
            ClassMember::Setter(x) => &x.span,
            ClassMember::Operator(x) => &x.span,
            ClassMember::PrimaryConstructorBody(x) => &x.span,
            ClassMember::Error(x) => &x.span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKeyword {
    /// Mutable declarations and bindings, with or without an explicit `var`.
    Var,
    Final,
    Const,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncMarker {
    Sync,
    SyncStar,
    Async,
    AsyncStar,
}

impl AsyncMarker {
    pub const fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::AsyncStar)
    }

    pub const fn is_generator(self) -> bool {
        matches!(self, Self::SyncStar | Self::AsyncStar)
    }
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub annotations: Vec<Annotation>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_external: bool,
    pub is_covariant: bool,
    pub is_late: bool,
    pub keyword: VarKeyword,
    pub field_type: Option<DartType>,
    pub declarators: Vec<VarDeclarator>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VarDeclarator {
    pub name: Identifier,
    pub initializer: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ConstructorDecl {
    pub annotations: Vec<Annotation>,
    pub is_const: bool,
    pub is_factory: bool,
    pub is_external: bool,
    pub name: Identifier,
    pub constructor_name: Option<Identifier>,
    pub params: FormalParamList,
    pub initializers: Vec<ConstructorInitializer>,
    /// Redirecting factory target: `factory C() = D;` / `= D.named;` / `= D<int>;`.
    pub redirect: Option<RedirectedConstructor>,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

/// The target of a redirecting factory constructor (`= Type[.name]`).
#[derive(Debug, Clone)]
pub struct RedirectedConstructor {
    pub type_: DartType,
    pub constructor_name: Option<Identifier>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ConstructorInitializer {
    SuperCall {
        call_name: Option<Identifier>,
        args: ArgList,
        span: Span,
    },
    ThisCall {
        call_name: Option<Identifier>,
        args: ArgList,
        span: Span,
    },
    FieldInit {
        field: Identifier,
        value: Expr,
        span: Span,
    },
    Assert {
        condition: Expr,
        message: Option<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub annotations: Vec<Annotation>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_external: bool,
    pub async_marker: AsyncMarker,
    pub return_type: Option<DartType>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub params: FormalParamList,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct GetterDecl {
    pub annotations: Vec<Annotation>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_external: bool,
    pub async_marker: AsyncMarker,
    pub return_type: Option<DartType>,
    pub name: Identifier,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SetterDecl {
    pub annotations: Vec<Annotation>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_external: bool,
    pub param_type: Option<DartType>,
    pub param_annotations: Vec<Annotation>,
    pub name: Identifier,
    pub param: Identifier,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct OperatorDecl {
    pub annotations: Vec<Annotation>,
    pub is_external: bool,
    pub return_type: Option<DartType>,
    pub op: String,
    pub params: FormalParamList,
    pub async_marker: AsyncMarker,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

impl OperatorDecl {
    /// The member name the operator is looked up by: unary minus is `unary-`,
    /// distinct from binary `-`.
    pub fn lookup_name(&self) -> &str {
        if self.op == "-" && self.params.params.is_empty() {
            "unary-"
        } else {
            &self.op
        }
    }
}

#[derive(Debug, Clone)]
pub struct MixinDecl {
    pub annotations: Vec<Annotation>,
    pub is_base: bool,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub on_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MixinClassDecl {
    pub annotations: Vec<Annotation>,
    pub is_abstract: bool,
    pub is_base: bool,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub primary_constructor: Option<PrimaryConstructor>,
    pub extends: Option<DartType>,
    pub with_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub annotations: Vec<Annotation>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    /// Dart 3.13 primary constructor in the header: `enum E(final int x)`.
    pub primary_constructor: Option<PrimaryConstructor>,
    pub with_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub variants: Vec<EnumVariant>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub annotations: Vec<Annotation>,
    pub name: Identifier,
    pub type_args: Vec<DartType>,
    /// Named (or `.new`) constructor invoked by an enhanced-enum constant, e.g.
    /// `a.foo(1)` — `Some(foo)`; `None` for an unnamed-constructor constant.
    pub constructor_name: Option<Identifier>,
    pub args: Option<ArgList>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExtensionDecl {
    pub annotations: Vec<Annotation>,
    pub name: Option<Identifier>,
    pub type_params: Vec<TypeParam>,
    pub on_type: DartType,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExtensionTypeDecl {
    pub annotations: Vec<Annotation>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    /// The representation constructor: `extension type const E._(int it)`.
    pub primary_constructor: PrimaryConstructor,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

impl MixinClassDecl {
    /// Declared members plus those of the primary constructor, if any.
    pub fn element_members(&self) -> EffectiveMembers<'_> {
        EffectiveMembers {
            primary: self.primary_constructor.as_ref(),
            declared: &self.members,
            representation: false,
        }
    }
}

impl EnumDecl {
    /// Declared members plus those of the primary constructor, if any.
    pub fn element_members(&self) -> EffectiveMembers<'_> {
        EffectiveMembers {
            primary: self.primary_constructor.as_ref(),
            declared: &self.members,
            representation: false,
        }
    }
}

impl ExtensionTypeDecl {
    /// Declared members plus the representation constructor and field.
    pub fn element_members(&self) -> EffectiveMembers<'_> {
        EffectiveMembers {
            primary: Some(&self.primary_constructor),
            declared: &self.members,
            representation: true,
        }
    }

    /// The representation field's declaring parameter (`int it` in
    /// `extension type E(int it)`); `None` only for malformed declarations.
    pub fn representation(&self) -> Option<&FormalParam> {
        self.primary_constructor.params.params.first()
    }
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub annotations: Vec<Annotation>,
    pub is_external: bool,
    pub async_marker: AsyncMarker,
    pub kind: FunctionKind,
    pub return_type: Option<DartType>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    Function(FormalParamList),
    Getter,
    Setter(FormalParamList),
}

impl FunctionDecl {
    pub fn params(&self) -> Option<&FormalParamList> {
        match &self.kind {
            FunctionKind::Function(params) | FunctionKind::Setter(params) => Some(params),
            FunctionKind::Getter => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TopLevelVarDecl {
    pub annotations: Vec<Annotation>,
    pub is_external: bool,
    pub keyword: VarKeyword,
    pub is_late: bool,
    pub var_type: Option<DartType>,
    pub declarators: Vec<VarDeclarator>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub annotations: Vec<Annotation>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    /// The aliased type. For a legacy `typedef R F(params);` this is the
    /// function type synthesized from the signature.
    pub aliased: DartType,
    /// The formal parameters of a legacy `typedef R F(params);` as written
    /// (the analyzer's `FunctionTypeAlias.parameters`); `None` for the
    /// `typedef F = Type;` form.
    pub legacy_params: Option<FormalParamList>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DartType {
    Named(NamedType),
    Function(Box<FunctionType>),
    Record(RecordType),
    Void { span: Span },
    Dynamic { span: Span },
}

impl DartType {
    pub fn span(&self) -> &Span {
        match self {
            DartType::Named(x) => &x.span,
            DartType::Function(x) => &x.span,
            DartType::Record(x) => &x.span,
            DartType::Void { span } => span,
            DartType::Dynamic { span } => span,
        }
    }

    pub fn is_nullable(&self) -> bool {
        match self {
            DartType::Named(x) => x.is_nullable,
            DartType::Function(x) => x.is_nullable,
            DartType::Record(x) => x.is_nullable,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamedType {
    /// Dot-separated name segments (e.g., `['foo', 'Bar']` for `foo.Bar`).
    pub segments: Vec<Identifier>,
    pub type_args: Vec<DartType>,
    pub is_nullable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub return_type: Option<Box<DartType>>,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<FunctionTypeParam>,
    pub is_nullable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionTypeParam {
    pub name: Option<Identifier>,
    pub param_type: DartType,
    /// True for required positional parameters and explicitly required named parameters.
    pub is_required: bool,
    pub is_named: bool,
}

#[derive(Debug, Clone)]
pub struct RecordType {
    pub positional: Vec<DartType>,
    pub named: Vec<NamedRecordField>,
    pub is_nullable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct NamedRecordField {
    pub name: Identifier,
    pub field_type: DartType,
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub annotations: Vec<Annotation>,
    pub name: Identifier,
    pub bound: Option<DartType>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FormalParamList {
    pub params: Vec<FormalParam>,
    pub span: Span,
}

impl FormalParamList {
    pub fn positional(&self) -> impl Iterator<Item = &FormalParam> {
        self.params
            .iter()
            .filter(|param| !matches!(param.kind, ParamKind::Named { .. }))
    }

    pub fn named(&self) -> impl Iterator<Item = &FormalParam> {
        self.params
            .iter()
            .filter(|param| matches!(param.kind, ParamKind::Named { .. }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Positional,
    OptionalPositional,
    Named { required: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializingFormal {
    This,
    Super,
}

#[derive(Debug, Clone)]
pub struct FormalParam {
    pub annotations: Vec<Annotation>,
    pub kind: ParamKind,
    pub is_covariant: bool,
    pub is_final: bool,
    /// Written with `var` (`C(var x)`); declaring in a primary constructor.
    pub is_var: bool,
    pub initializing: Option<InitializingFormal>,
    pub param_type: Option<DartType>,
    pub name: Identifier,
    pub default_value: Option<Expr>,
    /// Type parameters of a generic function-typed formal (`int cb<T>(T x)`).
    /// Empty unless `function_params` is present.
    pub type_params: Vec<TypeParam>,
    pub function_params: Option<FormalParamList>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum FunctionBody {
    Block(Block),
    Arrow(Box<Expr>, Span),
    Native(Option<StringLitNode>, Span),
}

impl FunctionBody {
    pub fn span(&self) -> &Span {
        match self {
            FunctionBody::Block(b) => &b.span,
            FunctionBody::Arrow(_, s) => s,
            FunctionBody::Native(_, s) => s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: Vec<Identifier>,
    /// Type arguments on the annotation: `@Native<int Function()>(...)`.
    pub type_args: Vec<DartType>,
    pub constructor_name: Option<Identifier>,
    pub args: Option<ArgList>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(Block),
    If(IfStmt),
    For(ForStmt),
    While(WhileStmt),
    DoWhile(DoWhileStmt),
    Switch(SwitchStmt),
    TryCatch(TryCatchStmt),
    Return(ReturnStmt),
    Throw(ThrowStmt),
    Rethrow(Span),
    Empty(Span),
    Break(BreakStmt),
    Continue(ContinueStmt),
    LocalVar(LocalVarDecl),
    /// Dart 3 pattern-variable declaration statement: `final (a, b) = expr;`,
    /// `var (x, :y) = expr;`, `final [a, b] = expr;`. Binds the identifiers in
    /// `pattern` (modeled as [`Pattern::Variable`]) to the destructured `init`.
    PatternDecl(PatternDeclaration),
    /// Dart 3 pattern-assignment statement: `(a, b) = expr;`, `[a, b] = expr;`.
    /// Assigns to already-declared variables via destructuring (no `var`/`final`
    /// keyword), distinct from [`Stmt::PatternDecl`].
    PatternAssign(PatternAssignStmt),
    /// A labeled statement: `label: stmt`. One or more labels may nest, each
    /// wrapping the labeled [`Stmt`] in its own `Labeled` node.
    Labeled(LabeledStmt),
    LocalFunc(LocalFuncDecl),
    Assert(AssertStmt),
    Yield(YieldStmt),
    Expr(ExprStmt),
    Error(ErrorNode),
}

impl Stmt {
    pub fn span(&self) -> &Span {
        match self {
            Stmt::Block(x) => &x.span,
            Stmt::If(x) => &x.span,
            Stmt::For(x) => &x.span,
            Stmt::While(x) => &x.span,
            Stmt::DoWhile(x) => &x.span,
            Stmt::Switch(x) => &x.span,
            Stmt::TryCatch(x) => &x.span,
            Stmt::Return(x) => &x.span,
            Stmt::Throw(x) => &x.span,
            Stmt::Rethrow(span) | Stmt::Empty(span) => span,
            Stmt::Break(x) => &x.span,
            Stmt::Continue(x) => &x.span,
            Stmt::LocalVar(x) => &x.span,
            Stmt::PatternDecl(x) => &x.span,
            Stmt::PatternAssign(x) => &x.span,
            Stmt::Labeled(x) => &x.span,
            Stmt::LocalFunc(x) => &x.span,
            Stmt::Assert(x) => &x.span,
            Stmt::Yield(x) => &x.span,
            Stmt::Expr(x) => &x.span,
            Stmt::Error(x) => &x.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: IfCondition,
    pub then_branch: Box<Stmt>,
    pub else_branch: Option<Box<Stmt>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum IfCondition {
    Expr(Expr),
    /// `if (<expr> case <pattern> [when <guard>])` — used by if-case statements
    /// and collection `if`-elements. The third field is the optional `when` guard.
    Case(Expr, Box<Pattern>, Option<Box<Expr>>),
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub is_await: bool,
    pub header: ForHeader,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForInit {
    VarDecl(LocalVarDecl),
    ForIn {
        annotations: Vec<Annotation>,
        /// False for `for (x in xs)`, which assigns an existing binding.
        declares_variable: bool,
        keyword: VarKeyword,
        var_type: Option<DartType>,
        name: Identifier,
        iterable: Box<Expr>,
    },
    /// Dart 3 pattern for-in loop variable: `for (final (i, s) in xs)`. The
    /// identifiers in `pattern` (modeled as [`Pattern::Variable`]) are the loop
    /// bindings.
    PatternForIn {
        annotations: Vec<Annotation>,
        keyword: VarKeyword,
        pattern: Box<Pattern>,
        iterable: Box<Expr>,
    },
    Exprs(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct ForHeader {
    pub init: Option<ForInit>,
    pub condition: Option<Expr>,
    pub updates: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DoWhileStmt {
    pub body: Box<Stmt>,
    pub condition: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SwitchStmt {
    pub subject: Expr,
    pub cases: Vec<SwitchCase>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SwitchCase {
    /// Statement labels preceding this case (`label: case 2:`), retained as
    /// `continue label;` targets.
    pub labels: Vec<Identifier>,
    /// One or more `case pattern:` / `default:` labels sharing this body.
    pub cases: Vec<SwitchCaseKind>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum SwitchCaseKind {
    /// `case <pattern> [when <guard>]:`
    Pattern(Box<Pattern>, Option<Box<Expr>>),
    /// `case <expression>:` in a library whose language version predates
    /// patterns (< 3.0) — the analyzer's `SwitchCase`.
    Expr(Box<Expr>),
    /// `default:`
    Default,
}

#[derive(Debug, Clone)]
pub struct TryCatchStmt {
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CatchClause {
    pub exception_type: Option<DartType>,
    pub exception_var: Option<Identifier>,
    pub stack_trace_var: Option<Identifier>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ThrowStmt {
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BreakStmt {
    pub label: Option<Identifier>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ContinueStmt {
    pub label: Option<Identifier>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LocalVarDecl {
    /// Metadata on a local declaration statement (`@pragma('x') final y = 1;`).
    pub annotations: Vec<Annotation>,
    pub keyword: VarKeyword,
    /// Exact token span of an explicit `var`, `final`, or `const` keyword.
    /// Keywordless typed declarations have no keyword span.
    pub keyword_span: Option<Span>,
    pub is_late: bool,
    pub var_type: Option<DartType>,
    pub declarators: Vec<VarDeclarator>,
    /// Syntax-only range, excluding leading documentation comments.
    pub span: Span,
    /// Full declaration range, including attached documentation comments.
    pub full_span: Span,
}

impl LocalVarDecl {
    pub fn range(&self) -> Span {
        self.span
    }

    pub fn full_range(&self) -> Span {
        self.full_span
    }
}

/// Dart 3 pattern-variable declaration: `(var|final) <pattern> = <init>`.
#[derive(Debug, Clone)]
pub struct PatternDeclaration {
    pub annotations: Vec<Annotation>,
    pub keyword: VarKeyword,
    /// Exact token span of the required `var` or `final` keyword.
    pub keyword_span: Span,
    pub pattern: Pattern,
    pub init: Expr,
    pub span: Span,
}

/// Dart 3 pattern-assignment statement: `<pattern> = <value>` (no `var`/`final`).
#[derive(Debug, Clone)]
pub struct PatternAssignStmt {
    pub pattern: Pattern,
    pub value: Expr,
    pub span: Span,
}

/// A labeled statement: `label: stmt`.
#[derive(Debug, Clone)]
pub struct LabeledStmt {
    pub label: Identifier,
    pub stmt: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LocalFuncDecl {
    pub annotations: Vec<Annotation>,
    pub async_marker: AsyncMarker,
    pub return_type: Option<DartType>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub params: FormalParamList,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssertStmt {
    pub condition: Expr,
    pub message: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct YieldStmt {
    pub is_star: bool,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit {
        value: String,
        span: Span,
    },
    DoubleLit {
        value: String,
        span: Span,
    },
    StringLit(StringLitNode),
    BoolLit {
        value: bool,
        span: Span,
    },
    NullLit {
        span: Span,
    },
    Ident(Identifier),
    Paren {
        expr: Box<Expr>,
        span: Span,
    },
    This {
        span: Span,
    },
    Super {
        span: Span,
    },

    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    PostfixIncDec {
        op: PostfixIncDec,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        op_span: Span,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
        span: Span,
    },
    Conditional {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    Is {
        expr: Box<Expr>,
        dart_type: DartType,
        negated: bool,
        span: Span,
    },
    As {
        expr: Box<Expr>,
        dart_type: DartType,
        span: Span,
    },

    Field {
        object: Box<Expr>,
        field: Identifier,
        is_null_safe: bool,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        is_null_safe: bool,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        type_args: Vec<DartType>,
        args: ArgList,
        span: Span,
    },
    Cascade {
        object: Box<Expr>,
        sections: Vec<CascadeSection>,
        /// True for a leading null-aware cascade `?..`.
        is_null_aware: bool,
        span: Span,
    },

    List {
        is_const: bool,
        type_arg: Option<DartType>,
        elements: Vec<CollectionElement>,
        span: Span,
    },
    Map {
        is_const: bool,
        type_args: Vec<DartType>,
        elements: Vec<MapElement>,
        span: Span,
    },
    Set {
        is_const: bool,
        type_arg: Option<DartType>,
        elements: Vec<CollectionElement>,
        span: Span,
    },

    Record {
        /// `const ('', X)` — a const record literal.
        is_const: bool,
        fields: Vec<RecordField>,
        span: Span,
    },

    FuncExpr {
        type_params: Vec<TypeParam>,
        params: FormalParamList,
        async_marker: AsyncMarker,
        body: Box<FunctionBody>,
        span: Span,
    },

    New {
        is_const: bool,
        dart_type: DartType,
        constructor_name: Option<Identifier>,
        args: ArgList,
        span: Span,
    },

    /// Static access shorthand (Dart 3.9); subsequent selectors wrap this node.
    DotShorthand {
        is_const: bool,
        name: Identifier,
        span: Span,
    },

    Await {
        expr: Box<Expr>,
        span: Span,
    },
    Throw {
        expr: Box<Expr>,
        span: Span,
    },

    Switch {
        subject: Box<Expr>,
        arms: Vec<SwitchExprArm>,
        span: Span,
    },

    NullAssert {
        operand: Box<Expr>,
        span: Span,
    },

    /// Symbol literal; `raw` includes the leading `#`.
    SymbolLit {
        raw: String,
        span: Span,
    },

    /// Bare generic tear-off instantiation: `identity<int>` (no call follows).
    GenericInstantiation {
        target: Box<Expr>,
        type_args: Vec<DartType>,
        span: Span,
    },

    Error {
        span: Span,
    },
}

impl Expr {
    /// The underlying expression, ignoring only explicit grouping parentheses.
    pub fn unparenthesized(&self) -> &Self {
        let mut expr = self;
        while let Self::Paren { expr: inner, .. } = expr {
            expr = inner;
        }
        expr
    }

    pub fn span(&self) -> &Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::DoubleLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::NullLit { span }
            | Expr::This { span }
            | Expr::Super { span }
            | Expr::Error { span } => span,
            Expr::StringLit(x) => &x.span,
            Expr::Ident(x) => &x.span,
            Expr::Unary { span, .. }
            | Expr::Paren { span, .. }
            | Expr::PostfixIncDec { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Assign { span, .. }
            | Expr::Conditional { span, .. }
            | Expr::Is { span, .. }
            | Expr::As { span, .. }
            | Expr::Field { span, .. }
            | Expr::Index { span, .. }
            | Expr::Call { span, .. }
            | Expr::Cascade { span, .. }
            | Expr::List { span, .. }
            | Expr::Map { span, .. }
            | Expr::Set { span, .. }
            | Expr::Record { span, .. }
            | Expr::FuncExpr { span, .. }
            | Expr::New { span, .. }
            | Expr::DotShorthand { span, .. }
            | Expr::Await { span, .. }
            | Expr::Throw { span, .. }
            | Expr::Switch { span, .. }
            | Expr::NullAssert { span, .. }
            | Expr::SymbolLit { span, .. }
            | Expr::GenericInstantiation { span, .. } => span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Minus,
    Bang,
    Tilde,
    PlusPlus,
    MinusMinus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixIncDec {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    IntDiv,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
    NullCoalesce,
}

impl BinaryOp {
    /// The name of the user-definable operator method this operator invokes;
    /// `None` for `&&`, `||` and `??`. `!=` invokes `==`.
    pub fn method_name(self) -> Option<&'static str> {
        Some(match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::IntDiv => "~/",
            Self::EqEq | Self::NotEq => "==",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::LtEq => "<=",
            Self::GtEq => ">=",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::UShr => ">>>",
            Self::And | Self::Or | Self::NullCoalesce => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Eq,
    PlusEq,
    MinusEq,
    MulEq,
    DivEq,
    ModEq,
    IntDivEq,
    AndEq,
    OrEq,
    XorEq,
    ShlEq,
    ShrEq,
    UShrEq,
    NullCoalesceEq,
}

impl AssignOp {
    /// The binary operator a compound assignment applies; `None` for `=`.
    pub fn binary(self) -> Option<BinaryOp> {
        Some(match self {
            Self::Eq => return None,
            Self::PlusEq => BinaryOp::Add,
            Self::MinusEq => BinaryOp::Sub,
            Self::MulEq => BinaryOp::Mul,
            Self::DivEq => BinaryOp::Div,
            Self::ModEq => BinaryOp::Mod,
            Self::IntDivEq => BinaryOp::IntDiv,
            Self::AndEq => BinaryOp::BitAnd,
            Self::OrEq => BinaryOp::BitOr,
            Self::XorEq => BinaryOp::BitXor,
            Self::ShlEq => BinaryOp::Shl,
            Self::ShrEq => BinaryOp::Shr,
            Self::UShrEq => BinaryOp::UShr,
            Self::NullCoalesceEq => BinaryOp::NullCoalesce,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CascadeSection {
    /// The selector chain in one `..`/`?..` section, e.g. `a.b()` in `..a.b()`.
    /// A plain single-selector section (`..a`) holds one op; chained selectors
    /// (`..a.b()`, `..m().n()`) hold one op per link, in source order. Any
    /// trailing assignment is the final `CascadeOp::Assign` or `CascadeOp::IndexAssign`.
    pub ops: Vec<CascadeOp>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CascadeOp {
    Field {
        name: Identifier,
        is_null_aware: bool,
    },
    Index {
        index: Box<Expr>,
        is_null_aware: bool,
        /// Complete bracket selector, including its closing delimiter.
        span: Span,
    },
    /// Assert that the value produced by the preceding selector is non-null.
    NullAssert {
        span: Span,
    },
    Call(Identifier, Vec<DartType>, ArgList),
    /// Invoke the value produced by the preceding cascade selector.
    Invoke(Vec<DartType>, ArgList),
    Assign(Box<Expr>, AssignOp, Box<Expr>),
    IndexAssign(Box<Expr>, AssignOp, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum CollectionElement {
    Expr(Expr),
    /// Dart 3.0 null-aware element `?expr` in a list or set literal: the element
    /// is omitted when `expr` evaluates to null. Preserves the inner `expr` so
    /// rules analyze it exactly like a plain element.
    NullAware {
        expr: Expr,
        span: Span,
    },
    Spread {
        expr: Expr,
        is_null_aware: bool,
        span: Span,
    },
    If {
        condition: IfCondition,
        then_elem: Box<CollectionElement>,
        else_elem: Option<Box<CollectionElement>>,
        span: Span,
    },
    For {
        /// `await for (...)` — an asynchronous for-in element in an async context.
        is_await: bool,
        header: ForHeader,
        element: Box<CollectionElement>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub key: Expr,
    pub value: Expr,
    /// Dart 3.0 null-aware key `?k: v` — the entry is omitted when the key is null.
    pub key_null_aware: bool,
    /// Dart 3.0 null-aware value `k: ?v` — the entry is omitted when the value is null.
    pub value_null_aware: bool,
    pub span: Span,
}

/// An element of a comprehension-form map literal (`{ for (..) k: v }`), mirroring
/// [`CollectionElement`] but with `k: v` [`MapEntry`] leaves instead of expressions.
#[derive(Debug, Clone)]
pub enum MapElement {
    Entry(MapEntry),
    Spread {
        expr: Expr,
        is_null_aware: bool,
        span: Span,
    },
    If {
        condition: IfCondition,
        then_entry: Box<MapElement>,
        else_entry: Option<Box<MapElement>>,
        span: Span,
    },
    For {
        /// `await for (...)` — an asynchronous for-in entry in an async context.
        is_await: bool,
        header: ForHeader,
        entry: Box<MapElement>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct RecordField {
    pub name: Option<Identifier>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArgList {
    pub positional: Vec<Expr>,
    pub named: Vec<NamedArg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct NamedArg {
    pub name: Identifier,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SwitchExprArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

// ── Patterns (Dart 3.x) ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard {
        type_: Option<DartType>,
        span: Span,
    },
    Variable {
        type_: Option<DartType>,
        name: Identifier,
        span: Span,
    },
    Literal(LiteralPattern),
    Const(ConstPattern),
    List(ListPattern),
    Record(RecordPattern),
    Map(MapPattern),
    Object(ObjectPattern),
    LogicalAnd {
        left: Box<Pattern>,
        right: Box<Pattern>,
        span: Span,
    },
    LogicalOr {
        left: Box<Pattern>,
        right: Box<Pattern>,
        span: Span,
    },
    Relational {
        op: RelationalPatternOp,
        value: Expr,
        span: Span,
    },
    Cast {
        inner: Box<Pattern>,
        cast_type: DartType,
        span: Span,
    },
    NullCheck {
        inner: Box<Pattern>,
        span: Span,
    },
    NullAssert {
        inner: Box<Pattern>,
        span: Span,
    },
    ParenPattern {
        inner: Box<Pattern>,
        span: Span,
    },
    Error {
        span: Span,
    },
}

impl Pattern {
    pub fn span(&self) -> &Span {
        match self {
            Pattern::Wildcard { span, .. }
            | Pattern::Variable { span, .. }
            | Pattern::LogicalAnd { span, .. }
            | Pattern::LogicalOr { span, .. }
            | Pattern::Relational { span, .. }
            | Pattern::Cast { span, .. }
            | Pattern::NullCheck { span, .. }
            | Pattern::NullAssert { span, .. }
            | Pattern::ParenPattern { span, .. }
            | Pattern::Error { span } => span,
            Pattern::Literal(x) => &x.span,
            Pattern::Const(x) => &x.span,
            Pattern::List(x) => &x.span,
            Pattern::Record(x) => &x.span,
            Pattern::Map(x) => &x.span,
            Pattern::Object(x) => &x.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiteralPattern {
    pub value: LiteralPatternValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LiteralPatternValue {
    Null,
    Bool(bool),
    Int(String),
    Double(String),
    String(StringLitNode),
    NegInt(String),
    NegDouble(String),
}

#[derive(Debug, Clone)]
pub struct ConstPattern {
    pub value: ConstPatternValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ConstPatternValue {
    Name(Vec<Identifier>),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct ListPattern {
    pub type_arg: Option<DartType>,
    pub elements: Vec<ListPatternElement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ListPatternElement {
    Pattern(Pattern),
    Rest(Option<Pattern>, Span),
}

#[derive(Debug, Clone)]
pub struct RecordPattern {
    pub fields: Vec<RecordPatternField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RecordPatternField {
    pub name: Option<Identifier>,
    pub pattern: Pattern,
    pub span: Span,
}

impl RecordPatternField {
    /// The explicit source label, excluding a shorthand's cloned binding name.
    pub fn source_name(&self) -> Option<&Identifier> {
        self.name
            .as_ref()
            .filter(|name| name.span.end <= self.pattern.span().start)
    }
}

#[derive(Debug, Clone)]
pub struct MapPattern {
    pub type_args: Vec<DartType>,
    pub entries: Vec<MapPatternEntry>,
    /// True when the map pattern contains a rest element (`...`).
    pub has_rest: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MapPatternEntry {
    pub key: Expr,
    pub pattern: Pattern,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectPattern {
    pub type_: DartType,
    pub fields: Vec<ObjectPatternField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectPatternField {
    pub name: Identifier,
    pub pattern: Option<Pattern>,
    pub span: Span,
}

impl ObjectPatternField {
    /// The explicit source label, excluding inferred names and recovery tokens.
    pub fn source_name(&self) -> Option<&Identifier> {
        let span = self.name.span;
        (self.span.start <= span.start
            && span.start < span.end
            && span.end <= self.span.end
            && self
                .pattern
                .as_ref()
                .is_none_or(|pattern| span.end <= pattern.span().start))
        .then_some(&self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationalPatternOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

// ── Error node ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ErrorNode {
    pub message: String,
    pub span: Span,
}

#[cfg(test)]
mod span_tests {
    use super::Span;

    #[test]
    fn merge_disjoint() {
        assert_eq!(Span::new(0, 10).merge(&Span::new(15, 25)), Span::new(0, 25));
    }

    #[test]
    fn merge_overlapping() {
        assert_eq!(Span::new(0, 15).merge(&Span::new(10, 20)), Span::new(0, 20));
    }

    #[test]
    fn merge_same() {
        let span = Span::new(5, 10);
        assert_eq!(span.merge(&span), span);
    }
}
