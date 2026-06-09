/// Classifies every AST node kind in the jdlint Dart AST.
///
/// Rules and visitors can use `SyntaxKind` to filter or dispatch on node types
/// without pattern-matching the full `Expr` / `Stmt` / `TopLevelDecl` enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SyntaxKind {
    // ── Top-level ─────────────────────────────────────────────────────────────
    Program,

    // ── Directives ────────────────────────────────────────────────────────────
    LibraryDirective,
    PartOfDirective,
    PartDirective,
    ImportDirective,
    ExportDirective,

    // ── Top-level declarations ────────────────────────────────────────────────
    ClassDecl,
    MixinDecl,
    MixinClassDecl,
    EnumDecl,
    ExtensionDecl,
    ExtensionTypeDecl,
    FunctionDecl,
    TopLevelVarDecl,
    TypeAliasDecl,

    // ── Class members ─────────────────────────────────────────────────────────
    FieldDecl,
    ConstructorDecl,
    MethodDecl,
    GetterDecl,
    SetterDecl,
    OperatorDecl,

    // ── Statements ────────────────────────────────────────────────────────────
    Block,
    IfStmt,
    ForStmt,
    WhileStmt,
    DoWhileStmt,
    SwitchStmt,
    TryCatchStmt,
    ReturnStmt,
    ThrowStmt,
    BreakStmt,
    ContinueStmt,
    LocalVarDecl,
    LocalFuncDecl,
    ExprStmt,
    AssertStmt,
    YieldStmt,

    // ── Expressions ───────────────────────────────────────────────────────────
    IdentifierExpr,
    IntLitExpr,
    DoubleLitExpr,
    StringLitExpr,
    BoolLitExpr,
    NullLitExpr,
    ThisExpr,
    SuperExpr,
    ListLitExpr,
    SetLitExpr,
    MapLitExpr,
    RecordLitExpr,
    CallExpr,
    IndexExpr,
    FieldAccessExpr,
    NullSafeFieldExpr,
    CascadeExpr,
    UnaryExpr,
    BinaryExpr,
    ConditionalExpr,
    AssignExpr,
    AsExpr,
    IsExpr,
    AwaitExpr,
    NewExpr,
    ThrowExpr,
    SwitchExpr,
    FunctionExpr,

    // ── Types ─────────────────────────────────────────────────────────────────
    NamedType,
    FunctionType,
    RecordType,
    VoidType,
    DynamicType,
    NeverType,
    InferredType,

    // ── Patterns (Dart 3.x) ───────────────────────────────────────────────────
    WildcardPattern,
    VariablePattern,
    LiteralPattern,
    ListPattern,
    RecordPattern,
    MapPattern,
    ObjectPattern,
    LogicalAndPattern,
    LogicalOrPattern,
    RelationalPattern,
    CastPattern,

    // ── Auxiliary ─────────────────────────────────────────────────────────────
    Annotation,
    Identifier,
    StringLit,
    FormalParamList,
    FormalParam,
    TypeParam,
    ErrorNode,
}

impl SyntaxKind {
    /// Returns `true` if this kind represents a declaration node.
    pub fn is_decl(self) -> bool {
        matches!(
            self,
            SyntaxKind::ClassDecl
                | SyntaxKind::MixinDecl
                | SyntaxKind::MixinClassDecl
                | SyntaxKind::EnumDecl
                | SyntaxKind::ExtensionDecl
                | SyntaxKind::ExtensionTypeDecl
                | SyntaxKind::FunctionDecl
                | SyntaxKind::TopLevelVarDecl
                | SyntaxKind::TypeAliasDecl
                | SyntaxKind::FieldDecl
                | SyntaxKind::ConstructorDecl
                | SyntaxKind::MethodDecl
                | SyntaxKind::GetterDecl
                | SyntaxKind::SetterDecl
                | SyntaxKind::OperatorDecl
        )
    }

    /// Returns `true` if this kind represents a statement node.
    pub fn is_stmt(self) -> bool {
        matches!(
            self,
            SyntaxKind::Block
                | SyntaxKind::IfStmt
                | SyntaxKind::ForStmt
                | SyntaxKind::WhileStmt
                | SyntaxKind::DoWhileStmt
                | SyntaxKind::SwitchStmt
                | SyntaxKind::TryCatchStmt
                | SyntaxKind::ReturnStmt
                | SyntaxKind::ThrowStmt
                | SyntaxKind::BreakStmt
                | SyntaxKind::ContinueStmt
                | SyntaxKind::LocalVarDecl
                | SyntaxKind::LocalFuncDecl
                | SyntaxKind::ExprStmt
                | SyntaxKind::AssertStmt
                | SyntaxKind::YieldStmt
        )
    }

    /// Returns `true` if this kind represents an expression node.
    pub fn is_expr(self) -> bool {
        matches!(
            self,
            SyntaxKind::IdentifierExpr
                | SyntaxKind::IntLitExpr
                | SyntaxKind::DoubleLitExpr
                | SyntaxKind::StringLitExpr
                | SyntaxKind::BoolLitExpr
                | SyntaxKind::NullLitExpr
                | SyntaxKind::ThisExpr
                | SyntaxKind::SuperExpr
                | SyntaxKind::ListLitExpr
                | SyntaxKind::SetLitExpr
                | SyntaxKind::MapLitExpr
                | SyntaxKind::RecordLitExpr
                | SyntaxKind::CallExpr
                | SyntaxKind::IndexExpr
                | SyntaxKind::FieldAccessExpr
                | SyntaxKind::NullSafeFieldExpr
                | SyntaxKind::CascadeExpr
                | SyntaxKind::UnaryExpr
                | SyntaxKind::BinaryExpr
                | SyntaxKind::ConditionalExpr
                | SyntaxKind::AssignExpr
                | SyntaxKind::AsExpr
                | SyntaxKind::IsExpr
                | SyntaxKind::AwaitExpr
                | SyntaxKind::NewExpr
                | SyntaxKind::ThrowExpr
                | SyntaxKind::SwitchExpr
                | SyntaxKind::FunctionExpr
        )
    }

    /// Returns `true` if this kind represents a pattern node.
    pub fn is_pattern(self) -> bool {
        matches!(
            self,
            SyntaxKind::WildcardPattern
                | SyntaxKind::VariablePattern
                | SyntaxKind::LiteralPattern
                | SyntaxKind::ListPattern
                | SyntaxKind::RecordPattern
                | SyntaxKind::MapPattern
                | SyntaxKind::ObjectPattern
                | SyntaxKind::LogicalAndPattern
                | SyntaxKind::LogicalOrPattern
                | SyntaxKind::RelationalPattern
                | SyntaxKind::CastPattern
        )
    }
}
