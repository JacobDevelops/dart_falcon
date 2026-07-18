/// Byte-range span for source attribution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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
    /// The raw source text (including quotes).
    pub raw: String,
    /// The decoded string value (without surrounding quotes, escapes resolved).
    pub value: String,
    pub span: Span,
    /// The parsed interpolated expressions (`$id` / `${expr}`) embedded in the
    /// literal, in source order. Empty for raw strings, non-interpolated
    /// literals, and any interpolation whose inner expression failed to parse
    /// cleanly (recorded conservatively as absent).
    pub interpolations: Vec<StringInterpolation>,
}

/// A single interpolation embedded in a [`StringLitNode`].
#[derive(Debug, Clone)]
pub struct StringInterpolation {
    /// The parsed interpolated expression.
    pub expr: Expr,
    /// Absolute byte range of the interpolated expression text in the original
    /// source: for `${e}` the range of `e` inside the braces, for `$ident` the
    /// range of `ident`.
    pub span: Span,
}

// ── Top-level compilation unit ────────────────────────────────────────────────

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

// ── Top-level declarations ─────────────────────────────────────────────────────

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

// ── Class ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ClassModifiers {
    pub is_abstract: bool,
    pub is_interface: bool,
    pub is_base: bool,
    pub is_final: bool,
    pub is_sealed: bool,
}

/// `[modifiers] class Name<T> [extends S] [with M] [implements I] { ... }`
#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub annotations: Vec<Annotation>,
    pub modifiers: ClassModifiers,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub extends: Option<DartType>,
    pub with_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
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
            ClassMember::Error(x) => &x.span,
        }
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
    pub is_final: bool,
    pub is_const: bool,
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
    pub is_async: bool,
    pub is_generator: bool,
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
    pub is_async: bool,
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
    pub is_async: bool,
    pub param_type: Option<DartType>,
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
    pub body: Option<FunctionBody>,
    pub span: Span,
}

// ── Mixin ─────────────────────────────────────────────────────────────────────

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
    pub extends: Option<DartType>,
    pub with_clause: Vec<DartType>,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

// ── Enum ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub annotations: Vec<Annotation>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
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
    pub args: Option<ArgList>,
    pub span: Span,
}

// ── Extension ─────────────────────────────────────────────────────────────────

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
    /// `extension type const Name(...)` — the representation constructor is const.
    pub is_const: bool,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub representation: ExtensionTypeRepresentation,
    pub implements: Vec<DartType>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExtensionTypeRepresentation {
    /// Named representation constructor: `extension type Name._(int it)`.
    pub constructor_name: Option<Identifier>,
    pub field_type: DartType,
    pub field_name: Identifier,
    pub span: Span,
}

// ── Top-level functions and variables ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub annotations: Vec<Annotation>,
    pub is_external: bool,
    pub is_async: bool,
    pub is_generator: bool,
    /// True when declared with `get` keyword (a top-level getter).
    pub is_getter: bool,
    /// True when declared with `set` keyword (a top-level setter).
    pub is_setter: bool,
    pub return_type: Option<DartType>,
    pub name: Identifier,
    pub type_params: Vec<TypeParam>,
    pub params: FormalParamList,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TopLevelVarDecl {
    pub annotations: Vec<Annotation>,
    pub is_external: bool,
    pub is_final: bool,
    pub is_const: bool,
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
    pub aliased: DartType,
    pub span: Span,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DartType {
    Named(NamedType),
    Function(Box<FunctionType>),
    Record(RecordType),
    Void { span: Span },
    Dynamic { span: Span },
    Never { span: Span },
}

impl DartType {
    pub fn span(&self) -> &Span {
        match self {
            DartType::Named(x) => &x.span,
            DartType::Function(x) => &x.span,
            DartType::Record(x) => &x.span,
            DartType::Void { span } => span,
            DartType::Dynamic { span } => span,
            DartType::Never { span } => span,
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

// ── Formal parameters ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FormalParamList {
    pub positional: Vec<FormalParam>,
    pub optional_positional: Vec<FormalParam>,
    pub named: Vec<FormalParam>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FormalParam {
    pub annotations: Vec<Annotation>,
    pub is_required: bool,
    pub is_covariant: bool,
    pub is_final: bool,
    pub is_field: bool,
    pub is_super: bool,
    pub param_type: Option<DartType>,
    pub name: Identifier,
    pub default_value: Option<Expr>,
    pub function_params: Option<FormalParamList>,
    pub span: Span,
}

// ── Function body ─────────────────────────────────────────────────────────────

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

// ── Annotations ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: Vec<Identifier>,
    /// Type arguments on the annotation: `@Native<int Function()>(...)`.
    pub type_args: Vec<DartType>,
    pub constructor_name: Option<Identifier>,
    pub args: Option<ArgList>,
    pub span: Span,
}

// ── Statements ────────────────────────────────────────────────────────────────

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
    pub init: Option<ForInit>,
    pub condition: Option<Expr>,
    pub update: Vec<Expr>,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForInit {
    VarDecl(LocalVarDecl),
    ForIn {
        is_final: bool,
        var_type: Option<DartType>,
        name: Identifier,
        iterable: Box<Expr>,
    },
    /// Dart 3 pattern for-in loop variable: `for (final (i, s) in xs)`. The
    /// identifiers in `pattern` (modeled as [`Pattern::Variable`]) are the loop
    /// bindings.
    PatternForIn {
        pattern: Box<Pattern>,
        iterable: Box<Expr>,
    },
    Exprs(Vec<Expr>),
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
    /// One or more `case pattern:` / `default:` labels sharing this body.
    pub cases: Vec<SwitchCaseKind>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum SwitchCaseKind {
    /// `case <pattern> [when <guard>]:`
    Pattern(Box<Pattern>, Box<Option<Expr>>),
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
    pub is_rethrow: bool,
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
    pub is_final: bool,
    pub is_const: bool,
    pub is_late: bool,
    pub var_type: Option<DartType>,
    pub declarators: Vec<VarDeclarator>,
    pub span: Span,
}

/// Dart 3 pattern-variable declaration: `(var|final) <pattern> = <init>`.
#[derive(Debug, Clone)]
pub struct PatternDeclaration {
    /// `final` keyword present (otherwise `var`).
    pub is_final: bool,
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
    pub is_async: bool,
    pub is_generator: bool,
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

// ── Expressions ───────────────────────────────────────────────────────────────

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
    This {
        span: Span,
    },
    Super {
        span: Span,
    },

    // Prefix unary
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    // Postfix (++ / --)
    PostfixIncDec {
        op: PostfixIncDec,
        operand: Box<Expr>,
        span: Span,
    },
    // Binary
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    // Assignment
    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
        span: Span,
    },
    // Conditional ternary
    Conditional {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    // Type tests
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

    // Member access
    Field {
        object: Box<Expr>,
        field: Identifier,
        is_null_safe: bool,
        span: Span,
    },
    // Index access
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        is_null_safe: bool,
        span: Span,
    },
    // Function / method call
    Call {
        callee: Box<Expr>,
        type_args: Vec<DartType>,
        args: ArgList,
        span: Span,
    },
    // Cascade
    Cascade {
        object: Box<Expr>,
        sections: Vec<CascadeSection>,
        /// True for a leading null-aware cascade `?..`.
        is_null_aware: bool,
        span: Span,
    },

    // Collections
    List {
        is_const: bool,
        type_arg: Option<DartType>,
        elements: Vec<CollectionElement>,
        span: Span,
    },
    Map {
        is_const: bool,
        type_args: Vec<DartType>,
        entries: Vec<MapEntry>,
        /// Comprehension form of a map literal (`{ for (..) k: v }`). A plain map
        /// uses `entries` and leaves this empty; a map containing any `for`/`if`/
        /// spread element puts *all* of its elements here (with `entries` empty).
        elements: Vec<MapElement>,
        span: Span,
    },
    Set {
        is_const: bool,
        type_arg: Option<DartType>,
        elements: Vec<CollectionElement>,
        span: Span,
    },

    // Records
    Record {
        fields: Vec<RecordField>,
        span: Span,
    },

    // Function expressions
    FuncExpr {
        type_params: Vec<TypeParam>,
        params: FormalParamList,
        is_async: bool,
        is_generator: bool,
        body: Box<FunctionBody>,
        span: Span,
    },

    // Instantiation / new
    New {
        is_const: bool,
        dart_type: DartType,
        constructor_name: Option<Identifier>,
        args: ArgList,
        span: Span,
    },

    // Static access shorthand (Dart 3.9):  `.name` / `const .name` / `.new`.
    // Only the head is captured here; any invocation or selector that follows
    // is represented by the wrapping `Call`/`Field`/etc. node.
    DotShorthand {
        is_const: bool,
        name: Identifier,
        span: Span,
    },

    // Await
    Await {
        expr: Box<Expr>,
        span: Span,
    },
    // Throw expression
    Throw {
        expr: Box<Expr>,
        span: Span,
    },

    // Switch expression (Dart 3.x)
    Switch {
        subject: Box<Expr>,
        arms: Vec<SwitchExprArm>,
        span: Span,
    },

    // Postfix null-assertion  expr!
    NullAssert {
        operand: Box<Expr>,
        span: Span,
    },

    // Symbol literal: `#name`, `#foo.bar`, `#+`. `raw` includes the leading `#`.
    SymbolLit {
        raw: String,
        span: Span,
    },

    // Bare generic tear-off instantiation: `identity<int>` (no call follows).
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

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Minus,
    Bang,
    Tilde,
    PlusPlus,
    MinusMinus,
}

#[derive(Debug, Clone)]
pub enum PostfixIncDec {
    Increment,
    Decrement,
}

#[derive(Debug, Clone)]
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
    IfNull, // alias for NullCoalesce; same thing
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct CascadeSection {
    pub op: CascadeOp,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CascadeOp {
    Field(Identifier, bool),
    Index(Box<Expr>, bool),
    Call(Identifier, Vec<DartType>, ArgList),
    Assign(Box<Expr>, AssignOp, Box<Expr>),
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
        /// Simple loop variable (`for (final x in xs)`). `None` when the header
        /// uses a Dart 3 pattern instead, in which case `pattern` is set.
        variable: Option<Identifier>,
        var_type: Option<DartType>,
        /// Dart 3 pattern-for header (`for (final (a, b) in xs)`).
        pattern: Option<Box<Pattern>>,
        iterable: Expr,
        element: Box<CollectionElement>,
        span: Span,
    },
    /// C-style comprehension header (`[for (var i = 0; i < n; i++) x]`).
    CFor {
        init: Option<ForInit>,
        condition: Option<Expr>,
        updates: Vec<Expr>,
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
        variable: Option<Identifier>,
        var_type: Option<DartType>,
        pattern: Option<Box<Pattern>>,
        iterable: Expr,
        entry: Box<MapElement>,
        span: Span,
    },
    /// C-style comprehension header (`{ for (var i = 0; i < n; i++) k: v }`).
    CFor {
        init: Option<ForInit>,
        condition: Option<Expr>,
        updates: Vec<Expr>,
        entry: Box<MapElement>,
        span: Span,
    },
}

/// Flatten every expression reachable from a comprehension map's `elements`
/// (`{ for (..) k: v }`, `{ if (c) k: v }`, `{ ...spread }`): entry keys and
/// values, spread expressions, `if` conditions, and `for` iterables, recursing
/// through nested `if`/`for` bodies.
///
/// Hand-rolled rule walkers that traverse `Expr::Map { entries }` should also
/// feed each expression this yields to the same walker, so a value hidden inside
/// a map comprehension is analyzed exactly like one in a plain entry. Walkers
/// built on [`crate::visitor::Visitor`] already descend into `elements` and do
/// not need this.
pub fn map_element_exprs(elements: &[MapElement]) -> Vec<&Expr> {
    let mut out = Vec::new();
    for element in elements {
        push_map_element_exprs(element, &mut out);
    }
    out
}

fn push_map_element_exprs<'a>(element: &'a MapElement, out: &mut Vec<&'a Expr>) {
    match element {
        MapElement::Entry(entry) => {
            out.push(&entry.key);
            out.push(&entry.value);
        }
        MapElement::Spread { expr, .. } => out.push(expr),
        MapElement::If {
            condition,
            then_entry,
            else_entry,
            ..
        } => {
            match condition {
                IfCondition::Expr(e) => out.push(e),
                IfCondition::Case(e, _, guard) => {
                    out.push(e);
                    if let Some(g) = guard {
                        out.push(g);
                    }
                }
            }
            push_map_element_exprs(then_entry, out);
            if let Some(else_entry) = else_entry {
                push_map_element_exprs(else_entry, out);
            }
        }
        MapElement::For {
            iterable, entry, ..
        } => {
            out.push(iterable);
            push_map_element_exprs(entry, out);
        }
        MapElement::CFor {
            init,
            condition,
            updates,
            entry,
            ..
        } => {
            push_for_init_exprs(init, out);
            if let Some(cond) = condition {
                out.push(cond);
            }
            for e in updates {
                out.push(e);
            }
            push_map_element_exprs(entry, out);
        }
    }
}

fn push_for_init_exprs<'a>(init: &'a Option<ForInit>, out: &mut Vec<&'a Expr>) {
    match init {
        Some(ForInit::VarDecl(d)) => {
            for decl in &d.declarators {
                if let Some(init) = &decl.initializer {
                    out.push(init);
                }
            }
        }
        Some(ForInit::ForIn { iterable, .. }) => out.push(iterable),
        Some(ForInit::PatternForIn { iterable, .. }) => out.push(iterable),
        Some(ForInit::Exprs(exprs)) => out.extend(exprs.iter()),
        None => {}
    }
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
    pub name: Vec<Identifier>,
    /// `None` for the dotted-name form (`const Foo.bar`); `Some` for a const
    /// constructor / collection / parenthesized expression form
    /// (`const Foo(1)`, `const [1, 2]`, `const (1 + 2)`).
    pub expr: Option<Box<Expr>>,
    pub span: Span,
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

#[derive(Debug, Clone)]
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
