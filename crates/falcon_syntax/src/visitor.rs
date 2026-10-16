use crate::ast::*;

/// Distinguishes block-parent statements from switch-case statement lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementListKind {
    Block,
    SwitchCase,
}

/// Visits declaration bodies without entering nested local functions or closures.
pub fn for_each_declared_body(
    program: &Program,
    source: &str,
    f: &mut impl FnMut(&FunctionBody, &Span, &Span),
) {
    struct Bodies<'a, F> {
        source: &'a str,
        callback: &'a mut F,
    }
    impl<F: FnMut(&FunctionBody, &Span, &Span)> Visitor for Bodies<'_, F> {
        fn visit_function_decl(&mut self, node: &FunctionDecl) {
            if let Some(body) = &node.body {
                (self.callback)(body, &node.name.span, &node.span);
            }
        }

        fn visit_class_member(&mut self, node: &ClassMember) {
            if let ClassMember::Operator(operator) = node {
                if let Some(body) = &operator.body {
                    let name = operator_header_span(operator, self.source);
                    (self.callback)(body, &name, &operator.span);
                }
                return;
            }
            let (body, name, span) = match node {
                ClassMember::Method(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Constructor(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Getter(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Setter(n) => (&n.body, &n.name.span, &n.span),
                ClassMember::Field(_) | ClassMember::Operator(_) | ClassMember::Error(_) => return,
                ClassMember::PrimaryConstructorBody(n) => {
                    if let Some(body) = &n.body {
                        (self.callback)(body, &n.this_span, &n.span);
                    }
                    return;
                }
            };
            if let Some(body) = body {
                (self.callback)(body, name, span);
            }
        }
    }
    Bodies {
        source,
        callback: f,
    }
    .visit_program(program);
}

// Suppression directives must target the operator token, not annotations or comments.
fn operator_header_span(node: &OperatorDecl, source: &str) -> Span {
    let start = node
        .annotations
        .last()
        .map_or(node.span.start, |a| a.span.end);
    let header = &source[start..node.params.span.start];
    let mut offset = 0;
    let mut keyword = start;
    while offset < header.len() {
        let rest = &header[offset..];
        if rest.starts_with("//") {
            offset += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            offset += 2;
            let mut depth = 1;
            while offset < header.len() && depth > 0 {
                if header[offset..].starts_with("/*") {
                    depth += 1;
                    offset += 2;
                } else if header[offset..].starts_with("*/") {
                    depth -= 1;
                    offset += 2;
                } else {
                    offset += header[offset..].chars().next().unwrap().len_utf8();
                }
            }
        } else {
            let ch = rest.chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                let end = rest
                    .char_indices()
                    .find(|(_, c)| !c.is_alphanumeric() && *c != '_' && *c != '$')
                    .map_or(rest.len(), |(i, _)| i);
                if &rest[..end] == "operator" {
                    keyword = start + offset;
                }
                offset += end;
            } else {
                offset += ch.len_utf8();
            }
        }
    }
    Span {
        start: keyword,
        end: (keyword + "operator".len()).min(node.params.span.start),
    }
}

/// Visits statement lists, including those inside local functions and closures.
pub fn for_each_stmt_list(program: &Program, f: &mut impl FnMut(&[Stmt], StatementListKind)) {
    struct Lists<'a, F>(&'a mut F);
    impl<F: FnMut(&[Stmt], StatementListKind)> Lists<'_, F> {
        fn body(&mut self, body: &FunctionBody) {
            if let FunctionBody::Block(block) = body {
                (self.0)(&block.stmts, StatementListKind::Block);
            }
        }
    }
    impl<F: FnMut(&[Stmt], StatementListKind)> Visitor for Lists<'_, F> {
        fn visit_function_decl(&mut self, node: &FunctionDecl) {
            if let Some(body) = &node.body {
                self.body(body);
            }
            walk_function_decl(self, node);
        }

        fn visit_class_member(&mut self, node: &ClassMember) {
            let body = match node {
                ClassMember::Method(n) => n.body.as_ref(),
                ClassMember::Constructor(n) => n.body.as_ref(),
                ClassMember::Getter(n) => n.body.as_ref(),
                ClassMember::Setter(n) => n.body.as_ref(),
                ClassMember::Operator(n) => n.body.as_ref(),
                ClassMember::PrimaryConstructorBody(n) => n.body.as_ref(),
                ClassMember::Field(_) | ClassMember::Error(_) => None,
            };
            if let Some(body) = body {
                self.body(body);
            }
            walk_class_member(self, node);
        }

        fn visit_stmt(&mut self, node: &Stmt) {
            match node {
                Stmt::Block(block) => (self.0)(&block.stmts, StatementListKind::Block),
                Stmt::LocalFunc(n) => self.body(&n.body),
                Stmt::TryCatch(n) => {
                    (self.0)(&n.body.stmts, StatementListKind::Block);
                    for catch in &n.catches {
                        (self.0)(&catch.body.stmts, StatementListKind::Block);
                    }
                    if let Some(block) = &n.finally {
                        (self.0)(&block.stmts, StatementListKind::Block);
                    }
                }
                Stmt::Switch(n) => {
                    for case in &n.cases {
                        (self.0)(&case.body, StatementListKind::SwitchCase);
                    }
                }
                Stmt::If(_)
                | Stmt::For(_)
                | Stmt::While(_)
                | Stmt::DoWhile(_)
                | Stmt::Return(_)
                | Stmt::Throw(_)
                | Stmt::LocalVar(_)
                | Stmt::PatternDecl(_)
                | Stmt::PatternAssign(_)
                | Stmt::Labeled(_)
                | Stmt::Assert(_)
                | Stmt::Yield(_)
                | Stmt::Expr(_)
                | Stmt::Break(_)
                | Stmt::Continue(_)
                | Stmt::Error(_)
                | Stmt::Rethrow(_)
                | Stmt::Empty(_) => {}
            }
            walk_stmt(self, node);
        }

        fn visit_expr(&mut self, node: &Expr) {
            if let Expr::FuncExpr { body, .. } = node {
                self.body(body);
            }
            walk_expr(self, node);
        }
    }
    Lists(f).visit_program(program);
}

/// A visitor that walks the Dart AST.
///
/// Structural hooks recurse through the shared `walk_*` helpers. Override a
/// hook and call its helper to continue traversal. Identifier/string hooks are
/// observations: every string container walks interpolation children separately,
/// even when `visit_string_lit` or `visit_native_string` is overridden.
///
/// `visit_name` and `visit_native_string` expose source children that historical
/// semantic visitors did not observe. They default to no-ops so adding complete
/// source observation does not change existing analyzer diagnostics.
///
/// Enum dispatch in the walkers is exhaustive: new AST variants must explicitly
/// define their traversal rather than falling through a wildcard arm.
pub trait Visitor: Sized {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_type_alias_decl(&mut self, node: &TypeAliasDecl) {
        walk_type_alias_decl(self, node);
    }

    fn visit_import(&mut self, node: &ImportDirective) {
        walk_import(self, node);
    }

    fn visit_export(&mut self, node: &ExportDirective) {
        walk_export(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        walk_top_level_decl(self, node);
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        walk_class_decl(self, node);
    }

    fn visit_mixin_decl(&mut self, node: &MixinDecl) {
        walk_mixin_decl(self, node);
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        walk_enum_decl(self, node);
    }

    fn visit_extension_decl(&mut self, node: &ExtensionDecl) {
        walk_extension_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        walk_class_member(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        walk_field_decl(self, node);
    }

    /// Observes each effective constructor without recursing; declaration
    /// walkers still visit its source children exactly once.
    fn visit_constructor(&mut self, _node: ConstructorRef<'_>) {}

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        walk_constructor_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        walk_getter_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        walk_setter_decl(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        walk_dart_type(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        walk_stmt(self, node);
    }

    fn visit_block(&mut self, node: &Block) {
        walk_block(self, node);
    }

    fn visit_function_body(&mut self, node: &FunctionBody) {
        walk_function_body(self, node);
    }

    fn visit_switch_case(&mut self, node: &SwitchCase) {
        walk_switch_case(self, node);
    }

    fn visit_catch_clause(&mut self, node: &CatchClause) {
        walk_catch_clause(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        walk_local_var(self, node);
    }

    /// The header of a `for` statement or a collection `for` element.
    fn visit_for_header(&mut self, node: &ForHeader) {
        walk_for_header(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        walk_expr(self, node);
    }

    fn visit_collection_element(&mut self, node: &CollectionElement) {
        walk_collection_element(self, node);
    }

    fn visit_map_element(&mut self, node: &MapElement) {
        walk_map_element(self, node);
    }

    fn visit_switch_expr_arm(&mut self, node: &SwitchExprArm) {
        walk_switch_expr_arm(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        walk_pattern(self, node);
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        walk_annotation(self, node);
    }

    fn visit_identifier(&mut self, _node: &Identifier) {}

    fn visit_string_lit(&mut self, _node: &StringLitNode) {}

    fn visit_formal_param(&mut self, node: &FormalParam) {
        walk_formal_param(self, node);
    }

    /// A primary constructor in a type header (`class C(var int x)`).
    fn visit_primary_constructor(&mut self, node: &PrimaryConstructor) {
        walk_primary_constructor(self, node);
    }

    /// The `this : ... { ... }` body part of a primary constructor.
    fn visit_primary_constructor_body(&mut self, node: &PrimaryConstructorBody) {
        walk_primary_constructor_body(self, node);
    }

    fn visit_library_directive(&mut self, node: &LibraryDirective) {
        walk_library_directive(self, node);
    }
    fn visit_part_of_directive(&mut self, node: &PartOfDirective) {
        walk_part_of_directive(self, node);
    }
    fn visit_part_directive(&mut self, node: &PartDirective) {
        walk_part_directive(self, node);
    }
    fn visit_configurable_uri(&mut self, node: &ConfigurableUri) {
        walk_configurable_uri(self, node);
    }
    fn visit_import_combinator(&mut self, node: &ImportCombinator) {
        walk_import_combinator(self, node);
    }
    fn visit_class_type_alias(&mut self, node: &ClassTypeAliasDecl) {
        walk_class_type_alias(self, node);
    }
    fn visit_mixin_class(&mut self, node: &MixinClassDecl) {
        walk_mixin_class(self, node);
    }
    fn visit_extension_type(&mut self, node: &ExtensionTypeDecl) {
        walk_extension_type(self, node);
    }
    fn visit_top_level_var(&mut self, node: &TopLevelVarDecl) {
        walk_top_level_var(self, node);
    }
    fn visit_operator_decl(&mut self, node: &OperatorDecl) {
        walk_operator_decl(self, node);
    }
    fn visit_constructor_initializer(&mut self, node: &ConstructorInitializer) {
        walk_constructor_initializer(self, node);
    }
    fn visit_arg_list(&mut self, node: &ArgList) {
        walk_arg_list(self, node);
    }
    fn visit_cascade_section(&mut self, node: &CascadeSection) {
        walk_cascade_section(self, node);
    }
    fn visit_for_init(&mut self, node: &ForInit) {
        walk_for_init(self, node);
    }
    fn visit_enum_variant(&mut self, node: &EnumVariant) {
        walk_enum_variant(self, node);
    }
    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
        walk_var_declarator(self, node);
    }
    fn visit_redirected_constructor(&mut self, node: &RedirectedConstructor) {
        walk_redirected_constructor(self, node);
    }
    fn visit_type_param(&mut self, node: &TypeParam) {
        walk_type_param(self, node);
    }
    fn visit_formal_param_list(&mut self, node: &FormalParamList) {
        walk_formal_param_list(self, node);
    }
    fn visit_named_type(&mut self, node: &NamedType) {
        walk_named_type(self, node);
    }
    fn visit_function_type(&mut self, node: &FunctionType) {
        walk_function_type(self, node);
    }
    fn visit_function_type_param(&mut self, node: &FunctionTypeParam) {
        walk_function_type_param(self, node);
    }
    fn visit_record_type(&mut self, node: &RecordType) {
        walk_record_type(self, node);
    }
    fn visit_named_record_field(&mut self, node: &NamedRecordField) {
        walk_named_record_field(self, node);
    }
    fn visit_if_condition(&mut self, node: &IfCondition) {
        walk_if_condition(self, node);
    }
    fn visit_if_stmt(&mut self, node: &IfStmt) {
        walk_if_stmt(self, node);
    }
    fn visit_for_stmt(&mut self, node: &ForStmt) {
        walk_for_stmt(self, node);
    }
    fn visit_while_stmt(&mut self, node: &WhileStmt) {
        walk_while_stmt(self, node);
    }
    fn visit_do_while_stmt(&mut self, node: &DoWhileStmt) {
        walk_do_while_stmt(self, node);
    }
    fn visit_switch_stmt(&mut self, node: &SwitchStmt) {
        walk_switch_stmt(self, node);
    }
    fn visit_try_catch_stmt(&mut self, node: &TryCatchStmt) {
        walk_try_catch_stmt(self, node);
    }
    fn visit_return_stmt(&mut self, node: &ReturnStmt) {
        walk_return_stmt(self, node);
    }
    fn visit_throw_stmt(&mut self, node: &ThrowStmt) {
        walk_throw_stmt(self, node);
    }
    fn visit_break_stmt(&mut self, node: &BreakStmt) {
        walk_break_stmt(self, node);
    }
    fn visit_continue_stmt(&mut self, node: &ContinueStmt) {
        walk_continue_stmt(self, node);
    }
    fn visit_pattern_declaration(&mut self, node: &PatternDeclaration) {
        walk_pattern_declaration(self, node);
    }
    fn visit_pattern_assign_stmt(&mut self, node: &PatternAssignStmt) {
        walk_pattern_assign_stmt(self, node);
    }
    fn visit_labeled_stmt(&mut self, node: &LabeledStmt) {
        walk_labeled_stmt(self, node);
    }
    fn visit_local_func_decl(&mut self, node: &LocalFuncDecl) {
        walk_local_func_decl(self, node);
    }
    fn visit_assert_stmt(&mut self, node: &AssertStmt) {
        walk_assert_stmt(self, node);
    }
    fn visit_yield_stmt(&mut self, node: &YieldStmt) {
        walk_yield_stmt(self, node);
    }
    fn visit_expr_stmt(&mut self, node: &ExprStmt) {
        walk_expr_stmt(self, node);
    }
    fn visit_error_node(&mut self, _node: &ErrorNode) {}
    fn visit_switch_case_kind(&mut self, node: &SwitchCaseKind) {
        walk_switch_case_kind(self, node);
    }
    fn visit_cascade_op(&mut self, node: &CascadeOp) {
        walk_cascade_op(self, node);
    }
    fn visit_map_entry(&mut self, node: &MapEntry) {
        walk_map_entry(self, node);
    }
    fn visit_record_field(&mut self, node: &RecordField) {
        walk_record_field(self, node);
    }
    fn visit_named_arg(&mut self, node: &NamedArg) {
        walk_named_arg(self, node);
    }
    fn visit_string_interpolation(&mut self, node: &StringInterpolation) {
        walk_string_interpolation(self, node);
    }
    fn visit_literal_pattern(&mut self, node: &LiteralPattern) {
        walk_literal_pattern(self, node);
    }
    fn visit_literal_pattern_value(&mut self, node: &LiteralPatternValue) {
        walk_literal_pattern_value(self, node);
    }
    fn visit_const_pattern(&mut self, node: &ConstPattern) {
        walk_const_pattern(self, node);
    }
    fn visit_const_pattern_value(&mut self, node: &ConstPatternValue) {
        walk_const_pattern_value(self, node);
    }
    fn visit_list_pattern(&mut self, node: &ListPattern) {
        walk_list_pattern(self, node);
    }
    fn visit_list_pattern_element(&mut self, node: &ListPatternElement) {
        walk_list_pattern_element(self, node);
    }
    fn visit_record_pattern(&mut self, node: &RecordPattern) {
        walk_record_pattern(self, node);
    }
    fn visit_record_pattern_field(&mut self, node: &RecordPatternField) {
        walk_record_pattern_field(self, node);
    }
    fn visit_map_pattern(&mut self, node: &MapPattern) {
        walk_map_pattern(self, node);
    }
    fn visit_map_pattern_entry(&mut self, node: &MapPatternEntry) {
        walk_map_pattern_entry(self, node);
    }
    fn visit_object_pattern(&mut self, node: &ObjectPattern) {
        walk_object_pattern(self, node);
    }
    fn visit_object_pattern_field(&mut self, node: &ObjectPatternField) {
        walk_object_pattern_field(self, node);
    }
    fn visit_function_kind(&mut self, node: &FunctionKind) {
        walk_function_kind(self, node);
    }
    /// Observes a source name not historically delivered to `visit_identifier`.
    fn visit_name(&mut self, _node: &Identifier) {}

    /// Observes the native-body literal without changing string diagnostics.
    fn visit_native_string(&mut self, _node: &StringLitNode) {}
}

// ── Walk helpers ──────────────────────────────────────────────────────────────

pub fn walk_program<V: Visitor>(v: &mut V, node: &Program) {
    if let Some(ref lib) = node.library_directive {
        v.visit_library_directive(lib);
    }
    if let Some(ref part_of) = node.part_of_directive {
        v.visit_part_of_directive(part_of);
    }
    for part in &node.part_directives {
        v.visit_part_directive(part);
    }
    for import in &node.imports {
        v.visit_import(import);
    }
    for export in &node.exports {
        v.visit_export(export);
    }
    for decl in &node.declarations {
        v.visit_top_level_decl(decl);
    }
}

pub fn walk_library_directive<V: Visitor>(v: &mut V, node: &LibraryDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    for seg in &node.name {
        v.visit_identifier(seg);
    }
}

pub fn walk_part_of_directive<V: Visitor>(v: &mut V, node: &PartOfDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref uri) = node.uri {
        walk_source_string(v, uri);
    }
    for seg in &node.name {
        v.visit_identifier(seg);
    }
}

pub fn walk_part_directive<V: Visitor>(v: &mut V, node: &PartDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    walk_source_string(v, &node.uri);
}

pub fn walk_import<V: Visitor>(v: &mut V, node: &ImportDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    walk_source_string(v, &node.uri);
    for cu in &node.configurable_uris {
        v.visit_configurable_uri(cu);
    }
    if let Some(ref alias) = node.as_name {
        v.visit_identifier(alias);
    }
    for comb in &node.combinators {
        v.visit_import_combinator(comb);
    }
}

pub fn walk_export<V: Visitor>(v: &mut V, node: &ExportDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    walk_source_string(v, &node.uri);
    for cu in &node.configurable_uris {
        v.visit_configurable_uri(cu);
    }
    for comb in &node.combinators {
        v.visit_import_combinator(comb);
    }
}

pub fn walk_configurable_uri<V: Visitor>(v: &mut V, node: &ConfigurableUri) {
    for seg in &node.test {
        v.visit_identifier(seg);
    }
    if let Some(ref value) = node.value {
        walk_source_string(v, value);
    }
    walk_source_string(v, &node.uri);
}

pub fn walk_import_combinator<V: Visitor>(v: &mut V, comb: &ImportCombinator) {
    match comb {
        ImportCombinator::Show(names, _) | ImportCombinator::Hide(names, _) => {
            for name in names {
                v.visit_identifier(name);
            }
        }
    }
}

pub fn walk_annotation<V: Visitor>(v: &mut V, node: &Annotation) {
    for seg in &node.name {
        v.visit_identifier(seg);
    }
    for t in &node.type_args {
        v.visit_dart_type(t);
    }
    if let Some(ref ctor) = node.constructor_name {
        v.visit_identifier(ctor);
    }
    if let Some(ref args) = node.args {
        v.visit_arg_list(args);
    }
}

/// Visits each annotated type parameter, its source name, and its bound.
pub fn walk_type_params<V: Visitor>(v: &mut V, type_params: &[TypeParam]) {
    for param in type_params {
        v.visit_type_param(param);
    }
}

pub fn walk_top_level_decl<V: Visitor>(v: &mut V, node: &TopLevelDecl) {
    match node {
        TopLevelDecl::Class(x) => v.visit_class_decl(x),
        TopLevelDecl::ClassTypeAlias(x) => v.visit_class_type_alias(x),
        TopLevelDecl::Mixin(x) => v.visit_mixin_decl(x),
        TopLevelDecl::MixinClass(x) => v.visit_mixin_class(x),
        TopLevelDecl::Enum(x) => v.visit_enum_decl(x),
        TopLevelDecl::Extension(x) => v.visit_extension_decl(x),
        TopLevelDecl::ExtensionType(x) => v.visit_extension_type(x),
        TopLevelDecl::Function(x) => v.visit_function_decl(x),
        TopLevelDecl::Variable(x) => v.visit_top_level_var(x),
        TopLevelDecl::TypeAlias(x) => v.visit_type_alias_decl(x),
        TopLevelDecl::Error(node) => v.visit_error_node(node),
    }
}

pub fn walk_class_decl<V: Visitor>(v: &mut V, node: &ClassDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    for constructor in node.element_members().constructors() {
        v.visit_constructor(constructor);
    }
    if let Some(ctor) = &node.primary_constructor {
        v.visit_primary_constructor(ctor);
    }
    if let Some(ref sup) = node.extends {
        v.visit_dart_type(sup);
    }
    for t in &node.with_clause {
        v.visit_dart_type(t);
    }
    for t in &node.implements {
        v.visit_dart_type(t);
    }
    for member in &node.members {
        v.visit_class_member(member);
    }
}

pub fn walk_class_type_alias<V: Visitor>(v: &mut V, node: &ClassTypeAliasDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    v.visit_dart_type(&node.superclass);
    for t in &node.with_clause {
        v.visit_dart_type(t);
    }
    for t in &node.implements {
        v.visit_dart_type(t);
    }
}

pub fn walk_mixin_decl<V: Visitor>(v: &mut V, node: &MixinDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    for t in &node.on_clause {
        v.visit_dart_type(t);
    }
    for t in &node.implements {
        v.visit_dart_type(t);
    }
    for member in &node.members {
        v.visit_class_member(member);
    }
}

pub fn walk_mixin_class<V: Visitor>(v: &mut V, node: &MixinClassDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    for constructor in node.element_members().constructors() {
        v.visit_constructor(constructor);
    }
    if let Some(ctor) = &node.primary_constructor {
        v.visit_primary_constructor(ctor);
    }
    if let Some(ref sup) = node.extends {
        v.visit_dart_type(sup);
    }
    for t in &node.with_clause {
        v.visit_dart_type(t);
    }
    for t in &node.implements {
        v.visit_dart_type(t);
    }
    for member in &node.members {
        v.visit_class_member(member);
    }
}

pub fn walk_enum_decl<V: Visitor>(v: &mut V, node: &EnumDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    for constructor in node.element_members().constructors() {
        v.visit_constructor(constructor);
    }
    if let Some(ctor) = &node.primary_constructor {
        v.visit_primary_constructor(ctor);
    }
    for t in &node.with_clause {
        v.visit_dart_type(t);
    }
    for t in &node.implements {
        v.visit_dart_type(t);
    }
    for variant in &node.variants {
        v.visit_enum_variant(variant);
    }
    for member in &node.members {
        v.visit_class_member(member);
    }
}

pub fn walk_extension_decl<V: Visitor>(v: &mut V, node: &ExtensionDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref name) = node.name {
        v.visit_identifier(name);
    }
    walk_type_params(v, &node.type_params);
    v.visit_dart_type(&node.on_type);
    for member in &node.members {
        v.visit_class_member(member);
    }
}

pub fn walk_extension_type<V: Visitor>(v: &mut V, node: &ExtensionTypeDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    for constructor in node.element_members().constructors() {
        v.visit_constructor(constructor);
    }
    v.visit_primary_constructor(&node.primary_constructor);
    for t in &node.implements {
        v.visit_dart_type(t);
    }
    for member in &node.members {
        v.visit_class_member(member);
    }
}

pub fn walk_function_decl<V: Visitor>(v: &mut V, node: &FunctionDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref ret) = node.return_type {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    v.visit_function_kind(&node.kind);
    if let Some(ref body) = node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_top_level_var<V: Visitor>(v: &mut V, node: &TopLevelVarDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref t) = node.var_type {
        v.visit_dart_type(t);
    }
    for decl in &node.declarators {
        v.visit_var_declarator(decl);
    }
}

pub fn walk_type_alias_decl<V: Visitor>(v: &mut V, node: &TypeAliasDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    // A legacy alias is walked as written: its return type and formal
    // parameters, not the function type synthesized from them.
    match &node.aliased {
        DartType::Function(function) if node.legacy_params.is_some() => {
            if let Some(ret) = &function.return_type {
                v.visit_dart_type(ret);
            }
            if let Some(params) = &node.legacy_params {
                v.visit_formal_param_list(params);
            }
        }
        DartType::Function(_)
        | DartType::Named(_)
        | DartType::Record(_)
        | DartType::Void { .. }
        | DartType::Dynamic { .. } => v.visit_dart_type(&node.aliased),
    }
}

pub fn walk_primary_constructor<V: Visitor>(v: &mut V, node: &PrimaryConstructor) {
    if let Some(name) = &node.constructor_name {
        v.visit_identifier(name);
    }
    v.visit_formal_param_list(&node.params);
}

pub fn walk_primary_constructor_body<V: Visitor>(v: &mut V, node: &PrimaryConstructorBody) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    for init in &node.initializers {
        v.visit_constructor_initializer(init);
    }
    if let Some(body) = &node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_class_member<V: Visitor>(v: &mut V, node: &ClassMember) {
    match node {
        ClassMember::Field(x) => v.visit_field_decl(x),
        ClassMember::Constructor(x) => v.visit_constructor_decl(x),
        ClassMember::Method(x) => v.visit_method_decl(x),
        ClassMember::Getter(x) => v.visit_getter_decl(x),
        ClassMember::Setter(x) => v.visit_setter_decl(x),
        ClassMember::Operator(x) => v.visit_operator_decl(x),
        ClassMember::PrimaryConstructorBody(x) => v.visit_primary_constructor_body(x),
        ClassMember::Error(node) => v.visit_error_node(node),
    }
}

pub fn walk_field_decl<V: Visitor>(v: &mut V, node: &FieldDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref t) = node.field_type {
        v.visit_dart_type(t);
    }
    for decl in &node.declarators {
        v.visit_var_declarator(decl);
    }
}

pub fn walk_constructor_decl<V: Visitor>(v: &mut V, node: &ConstructorDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    if let Some(name) = &node.constructor_name {
        v.visit_name(name);
    }
    v.visit_formal_param_list(&node.params);
    for init in &node.initializers {
        v.visit_constructor_initializer(init);
    }
    if let Some(ref redirect) = node.redirect {
        v.visit_redirected_constructor(redirect);
    }
    if let Some(ref body) = node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_constructor_initializer<V: Visitor>(v: &mut V, init: &ConstructorInitializer) {
    match init {
        ConstructorInitializer::SuperCall {
            call_name, args, ..
        }
        | ConstructorInitializer::ThisCall {
            call_name, args, ..
        } => {
            if let Some(name) = call_name {
                v.visit_identifier(name);
            }
            v.visit_arg_list(args);
        }
        ConstructorInitializer::FieldInit { field, value, .. } => {
            v.visit_identifier(field);
            v.visit_expr(value);
        }
        ConstructorInitializer::Assert {
            condition, message, ..
        } => {
            v.visit_expr(condition);
            if let Some(msg) = message {
                v.visit_expr(msg);
            }
        }
    }
}

pub fn walk_method_decl<V: Visitor>(v: &mut V, node: &MethodDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref ret) = node.return_type {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    v.visit_formal_param_list(&node.params);
    if let Some(ref body) = node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_getter_decl<V: Visitor>(v: &mut V, node: &GetterDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref ret) = node.return_type {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(&node.name);
    if let Some(ref body) = node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_setter_decl<V: Visitor>(v: &mut V, node: &SetterDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    for ann in &node.param_annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref t) = node.param_type {
        v.visit_dart_type(t);
    }
    v.visit_identifier(&node.name);
    v.visit_name(&node.param);
    if let Some(ref body) = node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_operator_decl<V: Visitor>(v: &mut V, node: &OperatorDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref ret) = node.return_type {
        v.visit_dart_type(ret);
    }
    v.visit_formal_param_list(&node.params);
    if let Some(ref body) = node.body {
        v.visit_function_body(body);
    }
}

pub fn walk_dart_type<V: Visitor>(v: &mut V, node: &DartType) {
    match node {
        DartType::Named(x) => v.visit_named_type(x),
        DartType::Function(x) => v.visit_function_type(x),
        DartType::Record(x) => v.visit_record_type(x),
        DartType::Void { .. } | DartType::Dynamic { .. } => {}
    }
}

pub fn walk_stmt<V: Visitor>(v: &mut V, node: &Stmt) {
    match node {
        Stmt::Block(x) => v.visit_block(x),
        Stmt::If(x) => v.visit_if_stmt(x),
        Stmt::For(x) => v.visit_for_stmt(x),
        Stmt::While(x) => v.visit_while_stmt(x),
        Stmt::DoWhile(x) => v.visit_do_while_stmt(x),
        Stmt::Switch(x) => v.visit_switch_stmt(x),
        Stmt::TryCatch(x) => v.visit_try_catch_stmt(x),
        Stmt::Return(x) => v.visit_return_stmt(x),
        Stmt::Throw(x) => v.visit_throw_stmt(x),
        Stmt::LocalVar(x) => v.visit_local_var(x),
        Stmt::PatternDecl(x) => v.visit_pattern_declaration(x),
        Stmt::PatternAssign(x) => v.visit_pattern_assign_stmt(x),
        Stmt::Labeled(x) => v.visit_labeled_stmt(x),
        Stmt::LocalFunc(x) => v.visit_local_func_decl(x),
        Stmt::Assert(x) => v.visit_assert_stmt(x),
        Stmt::Yield(x) => v.visit_yield_stmt(x),
        Stmt::Expr(x) => v.visit_expr_stmt(x),
        Stmt::Break(x) => v.visit_break_stmt(x),
        Stmt::Continue(x) => v.visit_continue_stmt(x),
        Stmt::Error(x) => v.visit_error_node(x),
        Stmt::Rethrow(_) | Stmt::Empty(_) => {}
    }
}

pub fn walk_expr<V: Visitor>(v: &mut V, node: &Expr) {
    match node {
        Expr::Paren { expr, .. } => v.visit_expr(expr),
        Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. }
        | Expr::This { .. }
        | Expr::Super { .. }
        | Expr::SymbolLit { .. }
        | Expr::Error { .. } => {}
        Expr::DotShorthand { name, .. } => v.visit_name(name),

        Expr::StringLit(x) => {
            walk_source_string(v, x);
        }
        Expr::Ident(x) => v.visit_identifier(x),

        Expr::Unary { operand, .. } => v.visit_expr(operand),
        Expr::PostfixIncDec { operand, .. } => v.visit_expr(operand),
        Expr::Binary { left, right, .. } => {
            v.visit_expr(left);
            v.visit_expr(right);
        }
        Expr::Assign { target, value, .. } => {
            v.visit_expr(target);
            v.visit_expr(value);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            v.visit_expr(condition);
            v.visit_expr(then_expr);
            v.visit_expr(else_expr);
        }
        Expr::Is {
            expr, dart_type, ..
        }
        | Expr::As {
            expr, dart_type, ..
        } => {
            v.visit_expr(expr);
            v.visit_dart_type(dart_type);
        }
        Expr::Field { object, field, .. } => {
            v.visit_expr(object);
            v.visit_name(field);
        }
        Expr::Index { object, index, .. } => {
            v.visit_expr(object);
            v.visit_expr(index);
        }
        Expr::Call {
            callee,
            type_args,
            args,
            ..
        } => {
            v.visit_expr(callee);
            for t in type_args {
                v.visit_dart_type(t);
            }
            v.visit_arg_list(args);
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            v.visit_expr(object);
            for section in sections {
                v.visit_cascade_section(section);
            }
        }
        Expr::List {
            type_arg, elements, ..
        }
        | Expr::Set {
            type_arg, elements, ..
        } => {
            if let Some(t) = type_arg {
                v.visit_dart_type(t);
            }
            for elem in elements {
                v.visit_collection_element(elem);
            }
        }
        Expr::Map {
            type_args,
            elements,
            ..
        } => {
            for t in type_args {
                v.visit_dart_type(t);
            }
            for element in elements {
                v.visit_map_element(element);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                v.visit_record_field(field);
            }
        }
        Expr::FuncExpr {
            type_params,
            params,
            body,
            ..
        } => {
            walk_type_params(v, type_params);
            v.visit_formal_param_list(params);
            v.visit_function_body(body);
        }
        Expr::New {
            dart_type,
            constructor_name,
            args,
            ..
        } => {
            v.visit_dart_type(dart_type);
            if let Some(name) = constructor_name {
                v.visit_name(name);
            }
            v.visit_arg_list(args);
        }
        Expr::GenericInstantiation {
            target, type_args, ..
        } => {
            v.visit_expr(target);
            for t in type_args {
                v.visit_dart_type(t);
            }
        }
        Expr::Await { expr, .. }
        | Expr::Throw { expr, .. }
        | Expr::NullAssert { operand: expr, .. } => v.visit_expr(expr),
        Expr::Switch { subject, arms, .. } => {
            v.visit_expr(subject);
            for arm in arms {
                v.visit_switch_expr_arm(arm);
            }
        }
    }
}

pub fn walk_pattern<V: Visitor>(v: &mut V, node: &Pattern) {
    match node {
        Pattern::Wildcard { type_, .. } => {
            if let Some(t) = type_ {
                v.visit_dart_type(t);
            }
        }
        Pattern::Variable { type_, name, .. } => {
            if let Some(t) = type_ {
                v.visit_dart_type(t);
            }
            v.visit_identifier(name);
        }
        Pattern::Literal(x) => v.visit_literal_pattern(x),
        Pattern::Const(x) => v.visit_const_pattern(x),
        Pattern::Error { .. } => {}
        Pattern::List(x) => v.visit_list_pattern(x),
        Pattern::Record(x) => v.visit_record_pattern(x),
        Pattern::Map(x) => v.visit_map_pattern(x),
        Pattern::Object(x) => v.visit_object_pattern(x),
        Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
            v.visit_pattern(left);
            v.visit_pattern(right);
        }
        Pattern::Relational { value, .. } => v.visit_expr(value),
        Pattern::Cast {
            inner, cast_type, ..
        } => {
            v.visit_pattern(inner);
            v.visit_dart_type(cast_type);
        }
        Pattern::NullCheck { inner, .. } | Pattern::NullAssert { inner, .. } => {
            v.visit_pattern(inner)
        }
        Pattern::ParenPattern { inner, .. } => v.visit_pattern(inner),
    }
}

/// Visits formal metadata, including nested function formals, without their defaults.
pub fn walk_formal_param_annotations<V: Visitor>(v: &mut V, node: &FormalParam) {
    for annotation in &node.annotations {
        v.visit_annotation(annotation);
    }
    walk_type_params(v, &node.type_params);
    if let Some(params) = &node.function_params {
        for param in &params.params {
            walk_formal_param_annotations(v, param);
        }
    }
}

pub fn walk_formal_param<V: Visitor>(v: &mut V, node: &FormalParam) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    walk_type_params(v, &node.type_params);
    if let Some(ref t) = node.param_type {
        v.visit_dart_type(t);
    }
    v.visit_identifier(&node.name);
    if let Some(ref params) = node.function_params {
        v.visit_formal_param_list(params);
    }
    if let Some(ref def) = node.default_value {
        v.visit_expr(def);
    }
}

/// Visits block contents without introducing a lexical scope.
pub fn walk_block<V: Visitor>(v: &mut V, node: &Block) {
    for stmt in &node.stmts {
        v.visit_stmt(stmt);
    }
}

pub fn walk_switch_case<V: Visitor>(v: &mut V, node: &SwitchCase) {
    for label in &node.labels {
        v.visit_identifier(label);
    }
    for kind in &node.cases {
        v.visit_switch_case_kind(kind);
    }
    for stmt in &node.body {
        v.visit_stmt(stmt);
    }
}

pub fn walk_catch_clause<V: Visitor>(v: &mut V, node: &CatchClause) {
    if let Some(t) = &node.exception_type {
        v.visit_dart_type(t);
    }
    if let Some(name) = &node.exception_var {
        v.visit_identifier(name);
    }
    if let Some(name) = &node.stack_trace_var {
        v.visit_identifier(name);
    }
    v.visit_block(&node.body);
}

pub fn walk_switch_expr_arm<V: Visitor>(v: &mut V, node: &SwitchExprArm) {
    v.visit_pattern(&node.pattern);
    if let Some(guard) = &node.guard {
        v.visit_expr(guard);
    }
    v.visit_expr(&node.body);
}

/// Visits body contents without introducing a lexical scope.
pub fn walk_function_body<V: Visitor>(v: &mut V, node: &FunctionBody) {
    match node {
        FunctionBody::Block(b) => v.visit_block(b),
        FunctionBody::Arrow(expr, _) => v.visit_expr(expr),
        FunctionBody::Native(literal, _) => {
            if let Some(literal) = literal {
                walk_native_string(v, literal);
            }
        }
    }
}

pub fn walk_arg_list<V: Visitor>(v: &mut V, node: &ArgList) {
    for arg in &node.positional {
        v.visit_expr(arg);
    }
    for arg in &node.named {
        v.visit_named_arg(arg);
    }
}

pub fn walk_cascade_section<V: Visitor>(v: &mut V, section: &CascadeSection) {
    for op in &section.ops {
        v.visit_cascade_op(op);
    }
}

pub fn walk_collection_element<V: Visitor>(v: &mut V, elem: &CollectionElement) {
    match elem {
        CollectionElement::Expr(e) => v.visit_expr(e),
        CollectionElement::NullAware { expr, .. } => v.visit_expr(expr),
        CollectionElement::Spread { expr, .. } => v.visit_expr(expr),
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            v.visit_if_condition(condition);
            v.visit_collection_element(then_elem);
            if let Some(else_e) = else_elem {
                v.visit_collection_element(else_e);
            }
        }
        CollectionElement::For {
            header, element, ..
        } => {
            v.visit_for_header(header);
            v.visit_collection_element(element);
        }
    }
}

pub fn walk_local_var<V: Visitor>(v: &mut V, node: &LocalVarDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(t) = &node.var_type {
        v.visit_dart_type(t);
    }
    for decl in &node.declarators {
        v.visit_var_declarator(decl);
    }
}

pub fn walk_for_init<V: Visitor>(v: &mut V, init: &ForInit) {
    match init {
        ForInit::VarDecl(d) => v.visit_local_var(d),
        ForInit::ForIn {
            annotations,
            var_type,
            name,
            iterable,
            ..
        } => {
            for annotation in annotations {
                v.visit_annotation(annotation);
            }
            if let Some(t) = var_type {
                v.visit_dart_type(t);
            }
            v.visit_identifier(name);
            v.visit_expr(iterable);
        }
        ForInit::PatternForIn {
            annotations,
            pattern,
            iterable,
            ..
        } => {
            for annotation in annotations {
                v.visit_annotation(annotation);
            }
            v.visit_pattern(pattern);
            v.visit_expr(iterable);
        }
        ForInit::Exprs(exprs) => {
            for e in exprs {
                v.visit_expr(e);
            }
        }
    }
}

pub fn walk_for_header<V: Visitor>(v: &mut V, header: &ForHeader) {
    if let Some(init) = &header.init {
        v.visit_for_init(init);
    }
    if let Some(condition) = &header.condition {
        v.visit_expr(condition);
    }
    for update in &header.updates {
        v.visit_expr(update);
    }
}

pub fn walk_map_element<V: Visitor>(v: &mut V, element: &MapElement) {
    match element {
        MapElement::Entry(entry) => v.visit_map_entry(entry),
        MapElement::Spread { expr, .. } => v.visit_expr(expr),
        MapElement::If {
            condition,
            then_entry,
            else_entry,
            ..
        } => {
            v.visit_if_condition(condition);
            v.visit_map_element(then_entry);
            if let Some(else_e) = else_entry {
                v.visit_map_element(else_e);
            }
        }
        MapElement::For { header, entry, .. } => {
            v.visit_for_header(header);
            v.visit_map_element(entry);
        }
    }
}

/// Visit `root` and every nested expression.
pub fn for_each_expr<F: FnMut(&Expr)>(root: &Expr, f: &mut F) {
    ExprWalker { f }.visit_expr(root);
}

/// Visit every expression in `program` in depth-first, parent-before-child order.
pub fn for_each_expr_in_program(program: &Program, mut f: impl FnMut(&Expr)) {
    ExprWalker { f: &mut f }.visit_program(program);
}

/// Visit every statement in `program` in depth-first, parent-before-child order.
pub fn for_each_stmt_in_program(program: &Program, f: impl FnMut(&Stmt)) {
    struct Statements<F>(F);
    impl<F: FnMut(&Stmt)> Visitor for Statements<F> {
        fn visit_stmt(&mut self, node: &Stmt) {
            (self.0)(node);
            walk_stmt(self, node);
        }
    }
    Statements(f).visit_program(program);
}

struct ExprWalker<'f, F> {
    f: &'f mut F,
}

impl<F: FnMut(&Expr)> Visitor for ExprWalker<'_, F> {
    fn visit_expr(&mut self, node: &Expr) {
        (self.f)(node);
        walk_expr(self, node);
    }
}

/// Every identifier a pattern binds — the `Pattern::Variable` leaves reached
/// through records, lists, maps, objects, and the unary pattern wrappers.
///
/// Rules that check declared names (length, casing, leading underscores) must
/// use this for Dart 3 destructuring: `final (a, b) = …` declares `a` and `b`
/// just as `final a = …` does. The match below is exhaustive on purpose — a new
/// pattern kind must fail to compile rather than silently bind invisible names.
pub fn bound_names(pattern: &Pattern) -> Vec<&Identifier> {
    let mut out = Vec::new();
    collect_bound_names(pattern, &mut out);
    out
}

fn collect_bound_names<'a>(pattern: &'a Pattern, out: &mut Vec<&'a Identifier>) {
    match pattern {
        Pattern::Variable { name, .. } => out.push(name),
        Pattern::List(list) => {
            for element in &list.elements {
                match element {
                    ListPatternElement::Pattern(p) | ListPatternElement::Rest(Some(p), _) => {
                        collect_bound_names(p, out)
                    }
                    ListPatternElement::Rest(None, _) => {}
                }
            }
        }
        Pattern::Record(record) => {
            for field in &record.fields {
                collect_bound_names(&field.pattern, out);
            }
        }
        Pattern::Map(map) => {
            for entry in &map.entries {
                collect_bound_names(&entry.pattern, out);
            }
        }
        Pattern::Object(object) => {
            for field in &object.fields {
                if let Some(p) = &field.pattern {
                    collect_bound_names(p, out);
                }
            }
        }
        Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
            collect_bound_names(left, out);
            collect_bound_names(right, out);
        }
        Pattern::Cast { inner, .. }
        | Pattern::NullCheck { inner, .. }
        | Pattern::NullAssert { inner, .. }
        | Pattern::ParenPattern { inner, .. } => collect_bound_names(inner, out),
        // Bind nothing: wildcards, literals, constants, relational tests, errors.
        Pattern::Wildcard { .. }
        | Pattern::Literal(_)
        | Pattern::Const(_)
        | Pattern::Relational { .. }
        | Pattern::Error { .. } => {}
    }
}

pub fn walk_enum_variant<V: Visitor>(v: &mut V, node: &EnumVariant) {
    for annotation in &node.annotations {
        v.visit_annotation(annotation);
    }
    v.visit_identifier(&node.name);
    for ty in &node.type_args {
        v.visit_dart_type(ty);
    }
    if let Some(name) = &node.constructor_name {
        v.visit_name(name);
    }
    if let Some(args) = &node.args {
        v.visit_arg_list(args);
    }
}

pub fn walk_var_declarator<V: Visitor>(v: &mut V, node: &VarDeclarator) {
    v.visit_identifier(&node.name);
    if let Some(init) = &node.initializer {
        v.visit_expr(init);
    }
}

pub fn walk_redirected_constructor<V: Visitor>(v: &mut V, node: &RedirectedConstructor) {
    v.visit_dart_type(&node.type_);
    if let Some(name) = &node.constructor_name {
        v.visit_identifier(name);
    }
}

pub fn walk_type_param<V: Visitor>(v: &mut V, node: &TypeParam) {
    for annotation in &node.annotations {
        v.visit_annotation(annotation);
    }
    v.visit_name(&node.name);
    if let Some(bound) = &node.bound {
        v.visit_dart_type(bound);
    }
}

pub fn walk_formal_param_list<V: Visitor>(v: &mut V, node: &FormalParamList) {
    for param in &node.params {
        v.visit_formal_param(param);
    }
}

pub fn walk_named_type<V: Visitor>(v: &mut V, node: &NamedType) {
    for segment in &node.segments {
        v.visit_name(segment);
    }
    for arg in &node.type_args {
        v.visit_dart_type(arg);
    }
}

pub fn walk_function_type<V: Visitor>(v: &mut V, node: &FunctionType) {
    walk_type_params(v, &node.type_params);
    if let Some(ret) = &node.return_type {
        v.visit_dart_type(ret);
    }
    for param in &node.params {
        v.visit_function_type_param(param);
    }
}

pub fn walk_function_type_param<V: Visitor>(v: &mut V, node: &FunctionTypeParam) {
    for annotation in &node.annotations {
        v.visit_annotation(annotation);
    }
    v.visit_dart_type(&node.param_type);
    if let Some(name) = &node.name {
        v.visit_name(name);
    }
}

pub fn walk_record_type<V: Visitor>(v: &mut V, node: &RecordType) {
    for ty in &node.positional {
        v.visit_dart_type(ty);
    }
    for name in node.positional_names.iter().flatten() {
        v.visit_name(name);
    }
    for field in &node.named {
        v.visit_named_record_field(field);
    }
}

pub fn walk_named_record_field<V: Visitor>(v: &mut V, node: &NamedRecordField) {
    v.visit_dart_type(&node.field_type);
    v.visit_name(&node.name);
}

pub fn walk_if_condition<V: Visitor>(v: &mut V, node: &IfCondition) {
    match node {
        IfCondition::Expr(expr) => v.visit_expr(expr),
        IfCondition::Case(expr, pattern, guard) => {
            v.visit_expr(expr);
            v.visit_pattern(pattern);
            if let Some(guard) = guard {
                v.visit_expr(guard);
            }
        }
    }
}

pub fn walk_if_stmt<V: Visitor>(v: &mut V, node: &IfStmt) {
    v.visit_if_condition(&node.condition);
    v.visit_stmt(&node.then_branch);
    if let Some(branch) = &node.else_branch {
        v.visit_stmt(branch);
    }
}

pub fn walk_for_stmt<V: Visitor>(v: &mut V, node: &ForStmt) {
    v.visit_for_header(&node.header);
    v.visit_stmt(&node.body);
}

pub fn walk_while_stmt<V: Visitor>(v: &mut V, node: &WhileStmt) {
    v.visit_expr(&node.condition);
    v.visit_stmt(&node.body);
}

pub fn walk_do_while_stmt<V: Visitor>(v: &mut V, node: &DoWhileStmt) {
    v.visit_stmt(&node.body);
    v.visit_expr(&node.condition);
}

pub fn walk_switch_stmt<V: Visitor>(v: &mut V, node: &SwitchStmt) {
    v.visit_expr(&node.subject);
    for case in &node.cases {
        v.visit_switch_case(case);
    }
}

pub fn walk_try_catch_stmt<V: Visitor>(v: &mut V, node: &TryCatchStmt) {
    v.visit_block(&node.body);
    for catch in &node.catches {
        v.visit_catch_clause(catch);
    }
    if let Some(finally) = &node.finally {
        v.visit_block(finally);
    }
}

pub fn walk_return_stmt<V: Visitor>(v: &mut V, node: &ReturnStmt) {
    if let Some(value) = &node.value {
        v.visit_expr(value);
    }
}

pub fn walk_throw_stmt<V: Visitor>(v: &mut V, node: &ThrowStmt) {
    v.visit_expr(&node.value);
}

pub fn walk_pattern_declaration<V: Visitor>(v: &mut V, node: &PatternDeclaration) {
    for annotation in &node.annotations {
        v.visit_annotation(annotation);
    }
    v.visit_pattern(&node.pattern);
    v.visit_expr(&node.init);
}

pub fn walk_pattern_assign_stmt<V: Visitor>(v: &mut V, node: &PatternAssignStmt) {
    v.visit_pattern(&node.pattern);
    v.visit_expr(&node.value);
}

pub fn walk_labeled_stmt<V: Visitor>(v: &mut V, node: &LabeledStmt) {
    v.visit_identifier(&node.label);
    v.visit_stmt(&node.stmt);
}

pub fn walk_local_func_decl<V: Visitor>(v: &mut V, node: &LocalFuncDecl) {
    for annotation in &node.annotations {
        v.visit_annotation(annotation);
    }
    if let Some(ret) = &node.return_type {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    v.visit_formal_param_list(&node.params);
    v.visit_function_body(&node.body);
}

pub fn walk_assert_stmt<V: Visitor>(v: &mut V, node: &AssertStmt) {
    v.visit_expr(&node.condition);
    if let Some(message) = &node.message {
        v.visit_expr(message);
    }
}

pub fn walk_yield_stmt<V: Visitor>(v: &mut V, node: &YieldStmt) {
    v.visit_expr(&node.value);
}

pub fn walk_expr_stmt<V: Visitor>(v: &mut V, node: &ExprStmt) {
    v.visit_expr(&node.expr);
}

pub fn walk_switch_case_kind<V: Visitor>(v: &mut V, node: &SwitchCaseKind) {
    match node {
        SwitchCaseKind::Pattern(pattern, guard) => {
            v.visit_pattern(pattern);
            if let Some(guard) = guard {
                v.visit_expr(guard);
            }
        }
        SwitchCaseKind::Expr(expr) => v.visit_expr(expr),
        SwitchCaseKind::Default => {}
    }
}

pub fn walk_cascade_op<V: Visitor>(v: &mut V, node: &CascadeOp) {
    match node {
        CascadeOp::Field { name, .. } => v.visit_identifier(name),
        CascadeOp::Index { index, .. } => v.visit_expr(index),
        CascadeOp::Call(ident, type_args, args) => {
            v.visit_identifier(ident);
            for ty in type_args {
                v.visit_dart_type(ty);
            }
            v.visit_arg_list(args);
        }
        CascadeOp::Invoke(type_args, args) => {
            for ty in type_args {
                v.visit_dart_type(ty);
            }
            v.visit_arg_list(args);
        }
        CascadeOp::Assign(target, _, value) | CascadeOp::IndexAssign(target, _, value) => {
            v.visit_expr(target);
            v.visit_expr(value);
        }
        CascadeOp::NullAssert { .. } => {}
    }
}

pub fn walk_map_entry<V: Visitor>(v: &mut V, node: &MapEntry) {
    v.visit_expr(&node.key);
    v.visit_expr(&node.value);
}

pub fn walk_record_field<V: Visitor>(v: &mut V, node: &RecordField) {
    if let Some(name) = &node.name {
        v.visit_name(name);
    }
    v.visit_expr(&node.value);
}

pub fn walk_named_arg<V: Visitor>(v: &mut V, node: &NamedArg) {
    v.visit_name(&node.name);
    v.visit_expr(&node.value);
}

pub fn walk_string_interpolation<V: Visitor>(v: &mut V, node: &StringInterpolation) {
    v.visit_expr(&node.expr);
}

pub fn walk_literal_pattern<V: Visitor>(v: &mut V, node: &LiteralPattern) {
    v.visit_literal_pattern_value(&node.value);
}

pub fn walk_literal_pattern_value<V: Visitor>(v: &mut V, node: &LiteralPatternValue) {
    match node {
        LiteralPatternValue::String(value) => walk_source_string(v, value),
        LiteralPatternValue::Null
        | LiteralPatternValue::Bool(_)
        | LiteralPatternValue::Int(_)
        | LiteralPatternValue::Double(_)
        | LiteralPatternValue::NegInt(_)
        | LiteralPatternValue::NegDouble(_) => {}
    }
}

pub fn walk_const_pattern<V: Visitor>(v: &mut V, node: &ConstPattern) {
    v.visit_const_pattern_value(&node.value);
}

pub fn walk_const_pattern_value<V: Visitor>(v: &mut V, node: &ConstPatternValue) {
    match node {
        ConstPatternValue::Name(segments) => {
            for segment in segments {
                v.visit_identifier(segment);
            }
        }
        ConstPatternValue::Expr(expr) => v.visit_expr(expr),
    }
}

pub fn walk_list_pattern<V: Visitor>(v: &mut V, node: &ListPattern) {
    if let Some(ty) = &node.type_arg {
        v.visit_dart_type(ty);
    }
    for element in &node.elements {
        v.visit_list_pattern_element(element);
    }
}

pub fn walk_list_pattern_element<V: Visitor>(v: &mut V, node: &ListPatternElement) {
    match node {
        ListPatternElement::Pattern(pattern) | ListPatternElement::Rest(Some(pattern), _) => {
            v.visit_pattern(pattern);
        }
        ListPatternElement::Rest(None, _) => {}
    }
}

pub fn walk_record_pattern<V: Visitor>(v: &mut V, node: &RecordPattern) {
    for field in &node.fields {
        v.visit_record_pattern_field(field);
    }
}

pub fn walk_record_pattern_field<V: Visitor>(v: &mut V, node: &RecordPatternField) {
    if let Some(name) = node.source_name() {
        v.visit_name(name);
    }
    v.visit_pattern(&node.pattern);
}

pub fn walk_map_pattern<V: Visitor>(v: &mut V, node: &MapPattern) {
    for ty in &node.type_args {
        v.visit_dart_type(ty);
    }
    for entry in &node.entries {
        v.visit_map_pattern_entry(entry);
    }
}

pub fn walk_map_pattern_entry<V: Visitor>(v: &mut V, node: &MapPatternEntry) {
    v.visit_expr(&node.key);
    v.visit_pattern(&node.pattern);
}

pub fn walk_object_pattern<V: Visitor>(v: &mut V, node: &ObjectPattern) {
    v.visit_dart_type(&node.type_);
    for field in &node.fields {
        v.visit_object_pattern_field(field);
    }
}

pub fn walk_object_pattern_field<V: Visitor>(v: &mut V, node: &ObjectPatternField) {
    if let Some(name) = node.source_name() {
        v.visit_name(name);
    }
    if let Some(pattern) = &node.pattern {
        v.visit_pattern(pattern);
    }
}

pub fn walk_function_kind<V: Visitor>(v: &mut V, node: &FunctionKind) {
    match node {
        FunctionKind::Function(params) | FunctionKind::Setter(params) => {
            v.visit_formal_param_list(params);
        }
        FunctionKind::Getter => {}
    }
}

pub fn walk_break_stmt<V: Visitor>(v: &mut V, node: &BreakStmt) {
    if let Some(label) = &node.label {
        v.visit_name(label);
    }
}

pub fn walk_continue_stmt<V: Visitor>(v: &mut V, node: &ContinueStmt) {
    if let Some(label) = &node.label {
        v.visit_name(label);
    }
}

/// Visits interpolation children separately from the string observation hook.
pub fn walk_string_lit<V: Visitor>(v: &mut V, node: &StringLitNode) {
    for interpolation in &node.interpolations {
        v.visit_string_interpolation(interpolation);
    }
}

/// Observes a source string and then visits all interpolation children.
pub fn walk_source_string<V: Visitor>(v: &mut V, node: &StringLitNode) {
    v.visit_string_lit(node);
    walk_string_lit(v, node);
}

/// Observes a native-body string and then visits all interpolation children.
pub fn walk_native_string<V: Visitor>(v: &mut V, node: &StringLitNode) {
    v.visit_native_string(node);
    walk_string_lit(v, node);
}
