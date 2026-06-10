# FALCON AST Format v1.0

`FALCON_AST_FORMAT_VERSION = "1.0"`

This document specifies the canonical shape of every AST node produced by
`falcon_dart_parser`. Any breaking change to enum variants or required struct
fields must bump the major version.

---

## Top-level types

| Type | Location |
|------|----------|
| `Program` | `ast.rs` |
| `Span` | `ast.rs` |
| `Identifier` | `ast.rs` |

### `Program`
```
Program { declarations: Vec<TopLevelDecl>, span: Span }
```

### `Span`
```
Span { start: usize, end: usize }
```

---

## Directives

| Variant | Struct |
|---------|--------|
| `TopLevelDecl::Import` | `ImportDirective` |
| `TopLevelDecl::Export` | `ExportDirective` |
| `TopLevelDecl::LibraryDirective` | `LibraryDirective` |
| `TopLevelDecl::PartOf` | `PartOfDirective` |
| `TopLevelDecl::Part` | `PartDirective` |

### `ImportDirective`
```
ImportDirective {
  annotations: Vec<Annotation>,
  uri: StringLitNode,
  is_deferred: bool,
  as_name: Option<Identifier>,
  combinators: Vec<ImportCombinator>,
  span: Span,
}
```

### `ImportCombinator`
```
ImportCombinator::Show(Vec<Identifier>, Span)
ImportCombinator::Hide(Vec<Identifier>, Span)
```

---

## Declarations

### `TopLevelDecl` variants
```
TopLevelDecl::Class(ClassDecl)
TopLevelDecl::Mixin(MixinDecl)
TopLevelDecl::MixinClass(MixinClassDecl)
TopLevelDecl::Enum(EnumDecl)
TopLevelDecl::Extension(ExtensionDecl)
TopLevelDecl::ExtensionType(ExtensionTypeDecl)
TopLevelDecl::Function(FunctionDecl)
TopLevelDecl::Variable(TopLevelVarDecl)
TopLevelDecl::TypeAlias(TypeAliasDecl)
TopLevelDecl::Error(ErrorNode)
```

### `ClassDecl`
```
ClassDecl {
  annotations: Vec<Annotation>,
  modifiers: ClassModifiers,
  name: Identifier,
  type_params: Vec<TypeParam>,
  extends: Option<DartType>,
  with_clause: Vec<DartType>,
  implements: Vec<DartType>,
  members: Vec<ClassMember>,
  span: Span,
}
```

### `ClassModifiers`
```
ClassModifiers {
  is_abstract: bool,
  is_interface: bool,
  is_base: bool,
  is_final: bool,
  is_sealed: bool,
}
```

### `ClassMember` variants
```
ClassMember::Field(FieldDecl)
ClassMember::Constructor(ConstructorDecl)
ClassMember::Method(MethodDecl)
ClassMember::Getter(GetterDecl)
ClassMember::Setter(SetterDecl)
ClassMember::Operator(OperatorDecl)
ClassMember::Error(ErrorNode)
```

### `FieldDecl`
```
FieldDecl {
  annotations: Vec<Annotation>,
  is_static: bool,
  is_abstract: bool,
  is_external: bool,
  is_covariant: bool,
  is_late: bool,
  is_final: bool,
  is_const: bool,
  field_type: Option<DartType>,
  declarators: Vec<VarDeclarator>,
  span: Span,
}
```

### `MethodDecl`
```
MethodDecl {
  annotations: Vec<Annotation>,
  is_static: bool,
  is_abstract: bool,
  is_external: bool,
  is_async: bool,
  is_generator: bool,
  return_type: Option<DartType>,
  name: Identifier,
  type_params: Vec<TypeParam>,
  params: FormalParamList,
  body: Option<FunctionBody>,
  span: Span,
}
```

### `ConstructorDecl`
```
ConstructorDecl {
  annotations: Vec<Annotation>,
  is_const: bool,
  is_factory: bool,
  is_external: bool,
  name: Identifier,
  constructor_name: Option<Identifier>,
  params: FormalParamList,
  initializers: Vec<ConstructorInitializer>,
  body: Option<FunctionBody>,
  span: Span,
}
```

### `GetterDecl`
```
GetterDecl {
  annotations: Vec<Annotation>,
  is_static: bool,
  is_abstract: bool,
  is_external: bool,
  is_async: bool,
  return_type: Option<DartType>,
  name: Identifier,
  body: Option<FunctionBody>,
  span: Span,
}
```

### `SetterDecl`
```
SetterDecl {
  annotations: Vec<Annotation>,
  is_static: bool,
  is_abstract: bool,
  is_external: bool,
  is_async: bool,
  param_type: Option<DartType>,
  name: Identifier,
  param: Identifier,
  body: Option<FunctionBody>,
  span: Span,
}
```

### `OperatorDecl`
```
OperatorDecl {
  annotations: Vec<Annotation>,
  is_external: bool,
  return_type: Option<DartType>,
  op: String,
  params: FormalParamList,
  body: Option<FunctionBody>,
  span: Span,
}
```

---

## Types

### `DartType` variants
```
DartType::Named(NamedType)
DartType::Function(Box<FunctionType>)
DartType::Record(RecordType)
DartType::Void { span }
DartType::Dynamic { span }
DartType::Never { span }
DartType::Inferred { span }
```

### `NamedType`
```
NamedType {
  segments: Vec<Identifier>,
  type_args: Vec<DartType>,
  is_nullable: bool,
  span: Span,
}
```

### `FunctionType`
```
FunctionType {
  return_type: Option<Box<DartType>>,
  type_params: Vec<TypeParam>,
  params: Vec<FunctionTypeParam>,
  is_nullable: bool,
  span: Span,
}
```

### `TypeParam`
```
TypeParam { name: Identifier, bound: Option<DartType>, span: Span }
```

---

## Parameters

### `FormalParamList`
```
FormalParamList {
  positional: Vec<FormalParam>,
  optional_positional: Vec<FormalParam>,
  named: Vec<FormalParam>,
  span: Span,
}
```

### `FormalParam`
```
FormalParam {
  annotations: Vec<Annotation>,
  is_required: bool,
  is_covariant: bool,
  is_final: bool,
  is_field: bool,
  is_super: bool,
  param_type: Option<DartType>,
  name: Identifier,
  default_value: Option<Expr>,
  function_params: Option<FormalParamList>,
  span: Span,
}
```

---

## Statements

### `Stmt` variants
```
Stmt::Block(Block)
Stmt::If(IfStmt)
Stmt::For(ForStmt)
Stmt::While(WhileStmt)
Stmt::DoWhile(DoWhileStmt)
Stmt::Switch(SwitchStmt)
Stmt::TryCatch(TryCatchStmt)
Stmt::Return(ReturnStmt)
Stmt::Throw(ThrowStmt)
Stmt::Break(BreakStmt)
Stmt::Continue(ContinueStmt)
Stmt::LocalVar(LocalVarDecl)
Stmt::LocalFunc(LocalFuncDecl)
Stmt::Expr(ExprStmt)
Stmt::Assert(AssertStmt)
Stmt::Yield(YieldStmt)
Stmt::Error(ErrorNode)
```

---

## Expressions

### `Expr` variants
```
Expr::Identifier(Identifier)
Expr::IntLit(IntLitNode)
Expr::DoubleLit(DoubleLitNode)
Expr::StringLit(StringLitNode)
Expr::BoolLit(BoolLitNode)
Expr::NullLit(NullLitNode)
Expr::This { span }
Expr::Super { span }
Expr::ListLit { type_arg, elements, is_const, span }
Expr::SetLit { type_arg, elements, is_const, span }
Expr::MapLit { type_args, entries, is_const, span }
Expr::RecordLit { fields, span }
Expr::Call { callee, type_args, args, span }
Expr::Index { target, index, span }
Expr::Field { target, name, span }
Expr::NullSafeField { target, name, span }
Expr::Cascade { target, operations, span }
Expr::Unary { op, operand, span }
Expr::Binary { left, op, right, span }
Expr::Conditional { condition, then_expr, else_expr, span }
Expr::Assign { target, op, value, span }
Expr::As { expr, dart_type, span }
Expr::Is { expr, dart_type, is_not, span }
Expr::Await { expr, span }
Expr::New { dart_type, constructor_name, args, span }
Expr::Throw { expr, span }
Expr::Switch { subject, arms, span }
Expr::Function { params, body, span }
Expr::Error(ErrorNode)
```

---

## Patterns (Dart 3.x)

### `Pattern` variants
```
Pattern::Wildcard { span }
Pattern::Variable { dart_type, name, span }
Pattern::Literal(Expr)
Pattern::List { type_arg, elements, span }
Pattern::Record { fields, span }
Pattern::Map { type_args, entries, span }
Pattern::Object { dart_type, fields, span }
Pattern::LogicalAnd { left, right, span }
Pattern::LogicalOr { left, right, span }
Pattern::Relational { op, operand, span }
Pattern::Cast { pattern, dart_type, span }
Pattern::Error(ErrorNode)
```

---

## Annotations

```
Annotation {
  name: Vec<Identifier>,
  args: Option<ArgList>,
  span: Span,
}
```

---

## Error Recovery

Parsers always return a complete AST. Unrecognized constructs produce
`*::Error(ErrorNode)` variants and parsing resumes at the next recovery
point (`;` or `}`).

```
ErrorNode { message: String, span: Span }
```
