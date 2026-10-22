use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementListKind {
    Block,
    SwitchCase,
}

pub fn declared_member_body<'a>(member: ClassMember<'a>) -> Option<(FunctionBody<'a>, Span)> {
    let (body, span) = match member {
        ClassMember::Method(n) => (n.body()?, n.span()),
        ClassMember::Constructor(n) => (n.body()?, n.span()),
        ClassMember::Getter(n) => (n.body()?, n.span()),
        ClassMember::Setter(n) => (n.body()?, n.span()),
        ClassMember::PrimaryConstructorBody(n) => (n.body()?, n.span()),
        ClassMember::Operator(n) => (n.body()?, n.span()),
        ClassMember::Field(_) | ClassMember::Error(_) => return None,
    };
    Some((body, span))
}

pub fn declared_member_name<'a>(member: ClassMember<'a>, _source: &str) -> Option<Span> {
    declared_member_body(member)?;
    Some(match member {
        ClassMember::Method(n) => n.name().span(),
        ClassMember::Constructor(n) => n.name().span(),
        ClassMember::Getter(n) => n.name().span(),
        ClassMember::Setter(n) => n.name().span(),
        ClassMember::PrimaryConstructorBody(n) => n.this_span(),
        ClassMember::Operator(n) => operator_header_span(n),
        ClassMember::Field(_) | ClassMember::Error(_) => return None,
    })
}
fn operator_header_span(node: OperatorDecl<'_>) -> Span {
    let start = node
        .annotations()
        .last()
        .map_or(node.span().start, |annotation| annotation.span().end);
    let end = node.params().span().start;
    let keyword = node
        .syntax()
        .significant_tokens()
        .rfind(|token| {
            token.kind() == crate::token::TokenKind::Operator
                && token.span().start >= start
                && token.span().start < end
        })
        .map_or(start, |token| token.span().start);
    Span {
        start: keyword,
        end: (keyword + "operator".len()).min(node.params().span().start),
    }
}

pub fn body_list<'a>(
    body: Option<FunctionBody<'a>>,
    f: &mut impl FnMut(AstChildren<'a, Stmt<'a>>, StatementListKind),
) {
    if let Some(FunctionBody::Block(block)) = body {
        f(block.stmts(), StatementListKind::Block);
    }
}

pub fn member_list<'a>(
    member: ClassMember<'a>,
    f: &mut impl FnMut(AstChildren<'a, Stmt<'a>>, StatementListKind),
) {
    body_list(declared_member_body(member).map(|(body, _)| body), f);
}

pub fn stmt_lists<'a>(
    node: Stmt<'a>,
    f: &mut impl FnMut(AstChildren<'a, Stmt<'a>>, StatementListKind),
) {
    match node {
        Stmt::Block(block) => f(block.stmts(), StatementListKind::Block),
        Stmt::LocalFunc(n) => body_list(Some(n.body()), f),
        Stmt::TryCatch(n) => {
            f(n.body().stmts(), StatementListKind::Block);
            for catch in n.catches() {
                f(catch.body().stmts(), StatementListKind::Block);
            }
            if let Some(block) = n.finally() {
                f(block.stmts(), StatementListKind::Block);
            }
        }
        Stmt::Switch(n) => {
            for case in n.cases() {
                f(case.body(), StatementListKind::SwitchCase);
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
}

pub fn for_each_stmt_list<'a>(
    program: Program<'a>,
    f: &mut impl FnMut(AstChildren<'a, Stmt<'a>>, StatementListKind),
) {
    struct Lists<'f, F>(&'f mut F);
    impl<'a, F: FnMut(AstChildren<'a, Stmt<'a>>, StatementListKind)> Visitor<'a> for Lists<'_, F> {
        fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
            body_list(node.body(), self.0);
            walk_function_decl(self, node);
        }

        fn visit_class_member(&mut self, node: ClassMember<'a>) {
            member_list(node, self.0);
            walk_class_member(self, node);
        }

        fn visit_stmt(&mut self, node: Stmt<'a>) {
            stmt_lists(node, self.0);
            walk_stmt(self, node);
        }

        fn visit_expr(&mut self, node: Expr<'a>) {
            walk_expr_chain(self, node, |visitor, node| {
                if let Expr::FuncExpr(node) = node {
                    body_list(Some(node.body()), visitor.0);
                }
                true
            });
        }
    }
    Lists(f).visit_program(program);
}
pub trait Visitor<'a>: Sized {
    fn visit_program(&mut self, node: Program<'a>) {
        walk_program(self, node);
    }

    fn visit_type_alias_decl(&mut self, node: TypeAliasDecl<'a>) {
        walk_type_alias_decl(self, node);
    }

    fn visit_import(&mut self, node: ImportDirective<'a>) {
        walk_import(self, node);
    }

    fn visit_export(&mut self, node: ExportDirective<'a>) {
        walk_export(self, node);
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        walk_top_level_decl(self, node);
    }

    fn visit_class_decl(&mut self, node: ClassDecl<'a>) {
        walk_class_decl(self, node);
    }

    fn visit_mixin_decl(&mut self, node: MixinDecl<'a>) {
        walk_mixin_decl(self, node);
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        walk_enum_decl(self, node);
    }

    fn visit_extension_decl(&mut self, node: ExtensionDecl<'a>) {
        walk_extension_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        walk_class_member(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        walk_field_decl(self, node);
    }
    fn visit_constructor(&mut self, _node: ConstructorRef<'a>) {}

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) {
        walk_constructor_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) {
        walk_getter_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) {
        walk_setter_decl(self, node);
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) {
        walk_dart_type(self, node);
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        walk_stmt(self, node);
    }

    fn visit_block(&mut self, node: Block<'a>) {
        walk_block(self, node);
    }

    fn visit_function_body(&mut self, node: FunctionBody<'a>) {
        walk_function_body(self, node);
    }

    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        walk_switch_case(self, node);
    }

    fn visit_catch_clause(&mut self, node: CatchClause<'a>) {
        walk_catch_clause(self, node);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) {
        walk_local_var(self, node);
    }
    fn visit_for_header(&mut self, node: ForHeader<'a>) {
        walk_for_header(self, node);
    }

    /// Walks expression chains without consuming the call stack. Overrides retain
    /// synchronous, recursive traversal when they delegate to [`walk_expr`].
    fn visit_expr(&mut self, node: Expr<'a>) {
        match node {
            Expr::Binary(_)
            | Expr::Assign(_)
            | Expr::Conditional(_)
            | Expr::PostfixIncDec(_)
            | Expr::NullAssert(_)
            | Expr::Is(_)
            | Expr::As(_)
            | Expr::Field(_)
            | Expr::Index(_)
            | Expr::Call(_)
            | Expr::GenericInstantiation(_)
            | Expr::Cascade(_) => walk_default_chain(self, Work::Expr(node)),
            _ => walk_expr(self, node),
        }
    }

    fn visit_collection_element(&mut self, node: CollectionElement<'a>) {
        walk_collection_element(self, node);
    }

    fn visit_map_element(&mut self, node: MapElement<'a>) {
        walk_map_element(self, node);
    }

    fn visit_switch_expr_arm(&mut self, node: SwitchExprArm<'a>) {
        walk_switch_expr_arm(self, node);
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) {
        match node {
            Pattern::LogicalOr(_)
            | Pattern::LogicalAnd(_)
            | Pattern::NullCheck(_)
            | Pattern::NullAssert(_)
            | Pattern::Cast(_) => walk_default_chain(self, Work::Pattern(node)),
            _ => walk_pattern(self, node),
        }
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        walk_annotation(self, node);
    }

    fn visit_identifier(&mut self, _node: Identifier<'a>) {}

    fn visit_string_lit(&mut self, _node: StringLitNode<'a>) {}

    fn visit_formal_param(&mut self, node: FormalParam<'a>) {
        walk_formal_param(self, node);
    }
    fn visit_primary_constructor(&mut self, node: PrimaryConstructor<'a>) {
        walk_primary_constructor(self, node);
    }
    fn visit_primary_constructor_body(&mut self, node: PrimaryConstructorBody<'a>) {
        walk_primary_constructor_body(self, node);
    }

    fn visit_library_directive(&mut self, node: LibraryDirective<'a>) {
        walk_library_directive(self, node);
    }
    fn visit_part_of_directive(&mut self, node: PartOfDirective<'a>) {
        walk_part_of_directive(self, node);
    }
    fn visit_part_directive(&mut self, node: PartDirective<'a>) {
        walk_part_directive(self, node);
    }
    fn visit_configurable_uri(&mut self, node: ConfigurableUri<'a>) {
        walk_configurable_uri(self, node);
    }
    fn visit_import_combinator(&mut self, node: ImportCombinator<'a>) {
        walk_import_combinator(self, node);
    }
    fn visit_class_type_alias(&mut self, node: ClassTypeAliasDecl<'a>) {
        walk_class_type_alias(self, node);
    }
    fn visit_mixin_class(&mut self, node: MixinClassDecl<'a>) {
        walk_mixin_class(self, node);
    }
    fn visit_extension_type(&mut self, node: ExtensionTypeDecl<'a>) {
        walk_extension_type(self, node);
    }
    fn visit_top_level_var(&mut self, node: TopLevelVarDecl<'a>) {
        walk_top_level_var(self, node);
    }
    fn visit_operator_decl(&mut self, node: OperatorDecl<'a>) {
        walk_operator_decl(self, node);
    }
    fn visit_constructor_initializer(&mut self, node: ConstructorInitializer<'a>) {
        walk_constructor_initializer(self, node);
    }
    fn visit_arg_list(&mut self, node: ArgList<'a>) {
        walk_arg_list(self, node);
    }
    fn visit_cascade_section(&mut self, node: CascadeSection<'a>) {
        walk_cascade_section(self, node);
    }
    fn visit_for_init(&mut self, node: ForInit<'a>) {
        walk_for_init(self, node);
    }
    fn visit_enum_variant(&mut self, node: EnumVariant<'a>) {
        walk_enum_variant(self, node);
    }
    fn visit_var_declarator(&mut self, node: VarDeclarator<'a>) {
        walk_var_declarator(self, node);
    }
    fn visit_redirected_constructor(&mut self, node: RedirectedConstructor<'a>) {
        walk_redirected_constructor(self, node);
    }
    fn visit_type_param(&mut self, node: TypeParam<'a>) {
        walk_type_param(self, node);
    }
    fn visit_formal_param_list(&mut self, node: FormalParamList<'a>) {
        walk_formal_param_list(self, node);
    }
    fn visit_named_type(&mut self, node: NamedType<'a>) {
        walk_named_type(self, node);
    }
    fn visit_function_type(&mut self, node: FunctionType<'a>) {
        walk_function_type(self, node);
    }
    fn visit_function_type_param(&mut self, node: FunctionTypeParam<'a>) {
        walk_function_type_param(self, node);
    }
    fn visit_record_type(&mut self, node: RecordType<'a>) {
        walk_record_type(self, node);
    }
    fn visit_named_record_field(&mut self, node: NamedRecordField<'a>) {
        walk_named_record_field(self, node);
    }
    fn visit_if_condition(&mut self, node: IfCondition<'a>) {
        walk_if_condition(self, node);
    }
    fn visit_if_stmt(&mut self, node: IfStmt<'a>) {
        walk_if_stmt(self, node);
    }
    fn visit_for_stmt(&mut self, node: ForStmt<'a>) {
        walk_for_stmt(self, node);
    }
    fn visit_while_stmt(&mut self, node: WhileStmt<'a>) {
        walk_while_stmt(self, node);
    }
    fn visit_do_while_stmt(&mut self, node: DoWhileStmt<'a>) {
        walk_do_while_stmt(self, node);
    }
    fn visit_switch_stmt(&mut self, node: SwitchStmt<'a>) {
        walk_switch_stmt(self, node);
    }
    fn visit_try_catch_stmt(&mut self, node: TryCatchStmt<'a>) {
        walk_try_catch_stmt(self, node);
    }
    fn visit_return_stmt(&mut self, node: ReturnStmt<'a>) {
        walk_return_stmt(self, node);
    }
    fn visit_throw_stmt(&mut self, node: ThrowStmt<'a>) {
        walk_throw_stmt(self, node);
    }
    fn visit_break_stmt(&mut self, node: BreakStmt<'a>) {
        walk_break_stmt(self, node);
    }
    fn visit_continue_stmt(&mut self, node: ContinueStmt<'a>) {
        walk_continue_stmt(self, node);
    }
    fn visit_pattern_declaration(&mut self, node: PatternDeclaration<'a>) {
        walk_pattern_declaration(self, node);
    }
    fn visit_pattern_assign_stmt(&mut self, node: PatternAssignStmt<'a>) {
        walk_pattern_assign_stmt(self, node);
    }
    fn visit_labeled_stmt(&mut self, node: LabeledStmt<'a>) {
        walk_labeled_stmt(self, node);
    }
    fn visit_local_func_decl(&mut self, node: LocalFuncDecl<'a>) {
        walk_local_func_decl(self, node);
    }
    fn visit_assert_stmt(&mut self, node: AssertStmt<'a>) {
        walk_assert_stmt(self, node);
    }
    fn visit_yield_stmt(&mut self, node: YieldStmt<'a>) {
        walk_yield_stmt(self, node);
    }
    fn visit_expr_stmt(&mut self, node: ExprStmt<'a>) {
        walk_expr_stmt(self, node);
    }
    fn visit_error_node(&mut self, _node: ErrorNode<'a>) {}
    fn visit_switch_case_kind(&mut self, node: SwitchCaseKind<'a>) {
        walk_switch_case_kind(self, node);
    }
    fn visit_cascade_op(&mut self, node: CascadeOp<'a>) {
        walk_cascade_op(self, node);
    }
    fn visit_map_entry(&mut self, node: MapEntry<'a>) {
        walk_map_entry(self, node);
    }
    fn visit_record_field(&mut self, node: RecordField<'a>) {
        walk_record_field(self, node);
    }
    fn visit_named_arg(&mut self, node: NamedArg<'a>) {
        walk_named_arg(self, node);
    }
    fn visit_string_interpolation(&mut self, node: StringInterpolation<'a>) {
        walk_string_interpolation(self, node);
    }
    fn visit_literal_pattern(&mut self, node: LiteralPattern<'a>) {
        walk_literal_pattern(self, node);
    }
    fn visit_literal_pattern_value(&mut self, node: LiteralPatternValue<'a>) {
        walk_literal_pattern_value(self, node);
    }
    fn visit_const_pattern(&mut self, node: ConstPattern<'a>) {
        walk_const_pattern(self, node);
    }
    fn visit_const_pattern_value(&mut self, node: ConstPatternValue<'a>) {
        walk_const_pattern_value(self, node);
    }
    fn visit_list_pattern(&mut self, node: ListPattern<'a>) {
        walk_list_pattern(self, node);
    }
    fn visit_list_pattern_element(&mut self, node: ListPatternElement<'a>) {
        walk_list_pattern_element(self, node);
    }
    fn visit_record_pattern(&mut self, node: RecordPattern<'a>) {
        walk_record_pattern(self, node);
    }
    fn visit_record_pattern_field(&mut self, node: RecordPatternField<'a>) {
        walk_record_pattern_field(self, node);
    }
    fn visit_map_pattern(&mut self, node: MapPattern<'a>) {
        walk_map_pattern(self, node);
    }
    fn visit_map_pattern_entry(&mut self, node: MapPatternEntry<'a>) {
        walk_map_pattern_entry(self, node);
    }
    fn visit_object_pattern(&mut self, node: ObjectPattern<'a>) {
        walk_object_pattern(self, node);
    }
    fn visit_object_pattern_field(&mut self, node: ObjectPatternField<'a>) {
        walk_object_pattern_field(self, node);
    }
    fn visit_function_kind(&mut self, node: FunctionKind<'a>) {
        walk_function_kind(self, node);
    }
    fn visit_name(&mut self, _node: Identifier<'a>) {}
    fn visit_native_string(&mut self, _node: StringLitNode<'a>) {}
}

pub fn walk_program<'a, V: Visitor<'a>>(v: &mut V, node: Program<'a>) {
    if let Some(lib) = node.library_directive() {
        v.visit_library_directive(lib);
    }
    if let Some(part_of) = node.part_of_directive() {
        v.visit_part_of_directive(part_of);
    }
    for part in node.part_directives() {
        v.visit_part_directive(part);
    }
    for import in node.imports() {
        v.visit_import(import);
    }
    for export in node.exports() {
        v.visit_export(export);
    }
    for decl in node.declarations() {
        v.visit_top_level_decl(decl);
    }
}

pub fn walk_library_directive<'a, V: Visitor<'a>>(v: &mut V, node: LibraryDirective<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    for seg in node.name() {
        v.visit_identifier(seg);
    }
}

pub fn walk_part_of_directive<'a, V: Visitor<'a>>(v: &mut V, node: PartOfDirective<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(uri) = node.uri() {
        walk_source_string(v, uri);
    }
    for seg in node.name() {
        v.visit_identifier(seg);
    }
}

pub fn walk_part_directive<'a, V: Visitor<'a>>(v: &mut V, node: PartDirective<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    walk_source_string(v, node.uri());
}

pub fn walk_import<'a, V: Visitor<'a>>(v: &mut V, node: ImportDirective<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    walk_source_string(v, node.uri());
    for cu in node.configurable_uris() {
        v.visit_configurable_uri(cu);
    }
    if let Some(alias) = node.as_name() {
        v.visit_identifier(alias);
    }
    for comb in node.combinators() {
        v.visit_import_combinator(comb);
    }
}

pub fn walk_export<'a, V: Visitor<'a>>(v: &mut V, node: ExportDirective<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    walk_source_string(v, node.uri());
    for cu in node.configurable_uris() {
        v.visit_configurable_uri(cu);
    }
    for comb in node.combinators() {
        v.visit_import_combinator(comb);
    }
}

pub fn walk_configurable_uri<'a, V: Visitor<'a>>(v: &mut V, node: ConfigurableUri<'a>) {
    for seg in node.test() {
        v.visit_identifier(seg);
    }
    if let Some(value) = node.value() {
        walk_source_string(v, value);
    }
    walk_source_string(v, node.uri());
}

pub fn walk_import_combinator<'a, V: Visitor<'a>>(v: &mut V, comb: ImportCombinator<'a>) {
    match comb {
        ImportCombinator::Show(names) | ImportCombinator::Hide(names) => {
            for name in names.names() {
                v.visit_identifier(name);
            }
        }
    }
}

pub fn walk_annotation<'a, V: Visitor<'a>>(v: &mut V, node: Annotation<'a>) {
    for seg in node.name() {
        v.visit_identifier(seg);
    }
    for t in node.type_args() {
        v.visit_dart_type(t);
    }
    if let Some(ctor) = node.constructor_name() {
        v.visit_identifier(ctor);
    }
    if let Some(args) = node.args() {
        v.visit_arg_list(args);
    }
}
pub fn walk_type_params<'a, V: Visitor<'a>>(
    v: &mut V,
    type_params: AstChildren<'a, TypeParam<'a>>,
) {
    for param in type_params {
        v.visit_type_param(param);
    }
}

pub fn walk_top_level_decl<'a, V: Visitor<'a>>(v: &mut V, node: TopLevelDecl<'a>) {
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

fn walk_class_header<'a, V: Visitor<'a>>(
    v: &mut V,
    node: SyntaxNode<'a>,
    name: Identifier<'a>,
    members: EffectiveMembers<'a>,
) {
    for annotation in AstChildren::new(node) {
        v.visit_annotation(annotation);
    }
    v.visit_identifier(name);
    walk_type_params(v, AstChildren::new(node));
    for constructor in members.constructors() {
        v.visit_constructor(constructor);
    }
    if let Some(constructor) = members.primary {
        v.visit_primary_constructor(constructor);
    }
}

pub fn walk_class_decl<'a, V: Visitor<'a>>(v: &mut V, node: ClassDecl<'a>) {
    walk_class_header(v, node.syntax(), node.name(), node.element_members());
    if let Some(sup) = node.extends() {
        v.visit_dart_type(sup);
    }
    for t in node.with_clause() {
        v.visit_dart_type(t);
    }
    for t in node.implements() {
        v.visit_dart_type(t);
    }
    for member in node.members() {
        v.visit_class_member(member);
    }
}

pub fn walk_class_type_alias<'a, V: Visitor<'a>>(v: &mut V, node: ClassTypeAliasDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    v.visit_identifier(node.name());
    walk_type_params(v, node.type_params());
    v.visit_dart_type(node.superclass());
    for t in node.with_clause() {
        v.visit_dart_type(t);
    }
    for t in node.implements() {
        v.visit_dart_type(t);
    }
}

pub fn walk_mixin_decl<'a, V: Visitor<'a>>(v: &mut V, node: MixinDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    v.visit_identifier(node.name());
    walk_type_params(v, node.type_params());
    for t in node.on_clause() {
        v.visit_dart_type(t);
    }
    for t in node.implements() {
        v.visit_dart_type(t);
    }
    for member in node.members() {
        v.visit_class_member(member);
    }
}

pub fn walk_mixin_class<'a, V: Visitor<'a>>(v: &mut V, node: MixinClassDecl<'a>) {
    walk_class_header(v, node.syntax(), node.name(), node.element_members());
    if let Some(sup) = node.extends() {
        v.visit_dart_type(sup);
    }
    for t in node.with_clause() {
        v.visit_dart_type(t);
    }
    for t in node.implements() {
        v.visit_dart_type(t);
    }
    for member in node.members() {
        v.visit_class_member(member);
    }
}

pub fn walk_enum_decl<'a, V: Visitor<'a>>(v: &mut V, node: EnumDecl<'a>) {
    walk_class_header(v, node.syntax(), node.name(), node.element_members());
    for t in node.with_clause() {
        v.visit_dart_type(t);
    }
    for t in node.implements() {
        v.visit_dart_type(t);
    }
    for variant in node.variants() {
        v.visit_enum_variant(variant);
    }
    for member in node.members() {
        v.visit_class_member(member);
    }
}

pub fn walk_extension_decl<'a, V: Visitor<'a>>(v: &mut V, node: ExtensionDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(name) = node.name() {
        v.visit_identifier(name);
    }
    walk_type_params(v, node.type_params());
    v.visit_dart_type(node.on_type());
    for member in node.members() {
        v.visit_class_member(member);
    }
}

pub fn walk_extension_type<'a, V: Visitor<'a>>(v: &mut V, node: ExtensionTypeDecl<'a>) {
    walk_class_header(v, node.syntax(), node.name(), node.element_members());
    for t in node.implements() {
        v.visit_dart_type(t);
    }
    for member in node.members() {
        v.visit_class_member(member);
    }
}

pub fn walk_function_decl<'a, V: Visitor<'a>>(v: &mut V, node: FunctionDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(ret) = node.return_type() {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(node.name());
    walk_type_params(v, node.type_params());
    v.visit_function_kind(node.kind());
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_top_level_var<'a, V: Visitor<'a>>(v: &mut V, node: TopLevelVarDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(t) = node.var_type() {
        v.visit_dart_type(t);
    }
    for decl in node.declarators() {
        v.visit_var_declarator(decl);
    }
}

pub fn walk_type_alias_decl<'a, V: Visitor<'a>>(v: &mut V, node: TypeAliasDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    v.visit_identifier(node.name());
    walk_type_params(v, node.type_params());
    match node.aliased() {
        DartType::Function(function) if node.legacy_params().is_some() => {
            if let Some(ret) = function.return_type() {
                v.visit_dart_type(ret);
            }
            if let Some(params) = node.legacy_params() {
                v.visit_formal_param_list(params);
            }
        }
        DartType::Function(_)
        | DartType::Named(_)
        | DartType::Record(_)
        | DartType::Void(_)
        | DartType::Dynamic(_) => v.visit_dart_type(node.aliased()),
    }
}

pub fn walk_primary_constructor<'a, V: Visitor<'a>>(v: &mut V, node: PrimaryConstructor<'a>) {
    if let Some(name) = node.constructor_name() {
        v.visit_identifier(name);
    }
    v.visit_formal_param_list(node.params());
}

pub fn walk_primary_constructor_body<'a, V: Visitor<'a>>(
    v: &mut V,
    node: PrimaryConstructorBody<'a>,
) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    for init in node.initializers() {
        v.visit_constructor_initializer(init);
    }
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_class_member<'a, V: Visitor<'a>>(v: &mut V, node: ClassMember<'a>) {
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

pub fn walk_field_decl<'a, V: Visitor<'a>>(v: &mut V, node: FieldDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(t) = node.field_type() {
        v.visit_dart_type(t);
    }
    for decl in node.declarators() {
        v.visit_var_declarator(decl);
    }
}

pub fn walk_constructor_decl<'a, V: Visitor<'a>>(v: &mut V, node: ConstructorDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    v.visit_identifier(node.name());
    if let Some(name) = node.constructor_name() {
        v.visit_name(name);
    }
    v.visit_formal_param_list(node.params());
    for init in node.initializers() {
        v.visit_constructor_initializer(init);
    }
    if let Some(redirect) = node.redirect() {
        v.visit_redirected_constructor(redirect);
    }
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_constructor_initializer<'a, V: Visitor<'a>>(
    v: &mut V,
    init: ConstructorInitializer<'a>,
) {
    match init {
        ConstructorInitializer::SuperCall(n) => {
            let call_name = n.call_name();
            let args = n.args();
            if let Some(name) = call_name {
                v.visit_identifier(name);
            }
            v.visit_arg_list(args);
        }
        ConstructorInitializer::ThisCall(n) => {
            let call_name = n.call_name();
            let args = n.args();
            if let Some(name) = call_name {
                v.visit_identifier(name);
            }
            v.visit_arg_list(args);
        }
        ConstructorInitializer::FieldInit(n) => {
            let field = n.field();
            let value = n.value();
            v.visit_identifier(field);
            v.visit_expr(value);
        }
        ConstructorInitializer::Assert(n) => {
            let condition = n.condition();
            let message = n.message();
            v.visit_expr(condition);
            if let Some(msg) = message {
                v.visit_expr(msg);
            }
        }
    }
}

pub fn walk_method_decl<'a, V: Visitor<'a>>(v: &mut V, node: MethodDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(ret) = node.return_type() {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(node.name());
    walk_type_params(v, node.type_params());
    v.visit_formal_param_list(node.params());
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_getter_decl<'a, V: Visitor<'a>>(v: &mut V, node: GetterDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(ret) = node.return_type() {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(node.name());
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_setter_decl<'a, V: Visitor<'a>>(v: &mut V, node: SetterDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    for ann in node.param_annotations() {
        v.visit_annotation(ann);
    }
    if let Some(t) = node.param_type() {
        v.visit_dart_type(t);
    }
    v.visit_identifier(node.name());
    v.visit_name(node.param());
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_operator_decl<'a, V: Visitor<'a>>(v: &mut V, node: OperatorDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(ret) = node.return_type() {
        v.visit_dart_type(ret);
    }
    v.visit_formal_param_list(node.params());
    if let Some(body) = node.body() {
        v.visit_function_body(body);
    }
}

pub fn walk_dart_type<'a, V: Visitor<'a>>(v: &mut V, node: DartType<'a>) {
    match node {
        DartType::Named(x) => v.visit_named_type(x),
        DartType::Function(x) => v.visit_function_type(x),
        DartType::Record(x) => v.visit_record_type(x),
        DartType::Void(_) | DartType::Dynamic(_) => {}
    }
}

pub fn walk_stmt<'a, V: Visitor<'a>>(v: &mut V, node: Stmt<'a>) {
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

#[derive(Clone, Copy)]
enum Work<'a> {
    Expr(Expr<'a>),
    Pattern(Pattern<'a>),
    Type(DartType<'a>),
    Name(Identifier<'a>),
    Call(CallExpr<'a>),
    TypeArgs(GenericInstantiationExpr<'a>),
    Cascade(CascadeExpr<'a>),
}

/// Walks expression chains iteratively, calling `enter` before each expression.
/// Returning `false` prunes that expression's children; scoped overrides can
/// instead visit those children synchronously before returning `false`.
pub fn walk_expr_chain<'a, V: Visitor<'a>>(
    visitor: &mut V,
    node: Expr<'a>,
    enter: impl FnMut(&mut V, Expr<'a>) -> bool,
) {
    walk_chain(visitor, Work::Expr(node), enter);
}

// Default hooks flatten chains; explicit walkers retain synchronous overrides.
#[inline(never)]
fn walk_default_chain<'a, V: Visitor<'a>>(v: &mut V, work: Work<'a>) {
    walk_chain(v, work, |_, _| true);
}

#[inline(never)]
fn walk_chain<'a, V: Visitor<'a>>(
    v: &mut V,
    mut work: Work<'a>,
    mut enter: impl FnMut(&mut V, Expr<'a>) -> bool,
) {
    struct WorkStack<'a> {
        inline: [Option<Work<'a>>; 4],
        len: usize,
        spill: Vec<Work<'a>>,
    }

    impl<'a> WorkStack<'a> {
        fn push(&mut self, work: Work<'a>) {
            if self.len < self.inline.len() {
                self.inline[self.len] = Some(work);
                self.len += 1;
            } else {
                self.spill.push(work);
            }
        }

        fn pop(&mut self) -> Option<Work<'a>> {
            if let Some(work) = self.spill.pop() {
                return Some(work);
            }
            self.len = self.len.checked_sub(1)?;
            self.inline[self.len].take()
        }
    }

    let mut pending = WorkStack {
        inline: [None; 4],
        len: 0,
        spill: Vec::new(),
    };
    loop {
        match work {
            Work::Expr(node) if enter(v, node) => {
                let child = match node {
                    Expr::Binary(n) => {
                        pending.push(Work::Expr(n.right()));
                        Some(n.left())
                    }
                    Expr::Assign(n) => {
                        pending.push(Work::Expr(n.value()));
                        Some(n.target())
                    }
                    Expr::Conditional(n) => {
                        pending.push(Work::Expr(n.else_expr()));
                        pending.push(Work::Expr(n.then_expr()));
                        Some(n.condition())
                    }
                    Expr::PostfixIncDec(n) => Some(n.operand()),
                    Expr::NullAssert(n) => Some(n.operand()),
                    Expr::Is(n) => {
                        pending.push(Work::Type(n.dart_type()));
                        Some(n.expr())
                    }
                    Expr::As(n) => {
                        pending.push(Work::Type(n.dart_type()));
                        Some(n.expr())
                    }
                    Expr::Field(n) => {
                        pending.push(Work::Name(n.field()));
                        Some(n.object())
                    }
                    Expr::Index(n) => {
                        pending.push(Work::Expr(n.index()));
                        Some(n.object())
                    }
                    Expr::Call(n) => {
                        pending.push(Work::Call(n));
                        Some(n.callee())
                    }
                    Expr::GenericInstantiation(n) => {
                        pending.push(Work::TypeArgs(n));
                        Some(n.target())
                    }
                    Expr::Cascade(n) => {
                        pending.push(Work::Cascade(n));
                        Some(n.object())
                    }
                    _ => {
                        walk_expr(v, node);
                        None
                    }
                };
                if let Some(child) = child {
                    work = Work::Expr(child);
                    continue;
                }
            }
            Work::Expr(_) => {}
            Work::Pattern(node) => {
                let child = match node {
                    Pattern::LogicalOr(n) => {
                        pending.push(Work::Pattern(n.right()));
                        Some(n.left())
                    }
                    Pattern::LogicalAnd(n) => {
                        pending.push(Work::Pattern(n.right()));
                        Some(n.left())
                    }
                    Pattern::NullCheck(n) => Some(n.inner()),
                    Pattern::NullAssert(n) => Some(n.inner()),
                    Pattern::Cast(n) => {
                        pending.push(Work::Type(n.cast_type()));
                        Some(n.inner())
                    }
                    _ => {
                        walk_pattern(v, node);
                        None
                    }
                };
                if let Some(child) = child {
                    work = Work::Pattern(child);
                    continue;
                }
            }
            Work::Type(node) => v.visit_dart_type(node),
            Work::Name(node) => v.visit_name(node),
            Work::Call(node) => {
                for ty in node.type_args() {
                    v.visit_dart_type(ty);
                }
                v.visit_arg_list(node.args());
            }
            Work::TypeArgs(node) => {
                for ty in node.type_args() {
                    v.visit_dart_type(ty);
                }
            }
            Work::Cascade(node) => {
                for section in node.sections() {
                    v.visit_cascade_section(section);
                }
            }
        }
        let Some(next) = pending.pop() else {
            break;
        };
        work = next;
    }
}

pub fn walk_expr<'a, V: Visitor<'a>>(v: &mut V, node: Expr<'a>) {
    match node {
        Expr::Paren(n) => v.visit_expr(n.expr()),
        Expr::IntLit(_)
        | Expr::DoubleLit(_)
        | Expr::BoolLit(_)
        | Expr::NullLit(_)
        | Expr::This(_)
        | Expr::Super(_)
        | Expr::SymbolLit(_)
        | Expr::Error(_) => {}
        Expr::DotShorthand(n) => v.visit_name(n.name()),

        Expr::StringLit(x) => walk_source_string(v, x),
        Expr::Ident(x) => v.visit_identifier(x),

        Expr::Unary(n) => v.visit_expr(n.operand()),
        Expr::PostfixIncDec(n) => v.visit_expr(n.operand()),
        Expr::Binary(n) => {
            v.visit_expr(n.left());
            v.visit_expr(n.right());
        }
        Expr::Assign(n) => {
            v.visit_expr(n.target());
            v.visit_expr(n.value());
        }
        Expr::Conditional(n) => {
            v.visit_expr(n.condition());
            v.visit_expr(n.then_expr());
            v.visit_expr(n.else_expr());
        }
        Expr::Is(n) => {
            v.visit_expr(n.expr());
            v.visit_dart_type(n.dart_type());
        }
        Expr::As(n) => {
            v.visit_expr(n.expr());
            v.visit_dart_type(n.dart_type());
        }
        Expr::Field(n) => {
            v.visit_expr(n.object());
            v.visit_name(n.field());
        }
        Expr::Index(n) => {
            v.visit_expr(n.object());
            v.visit_expr(n.index());
        }
        Expr::Call(n) => {
            v.visit_expr(n.callee());
            for t in n.type_args() {
                v.visit_dart_type(t);
            }
            v.visit_arg_list(n.args());
        }
        Expr::Cascade(n) => {
            v.visit_expr(n.object());
            for section in n.sections() {
                v.visit_cascade_section(section);
            }
        }
        Expr::List(n) => {
            if let Some(t) = n.type_arg() {
                v.visit_dart_type(t);
            }
            for elem in n.elements() {
                v.visit_collection_element(elem);
            }
        }
        Expr::Set(n) => {
            if let Some(t) = n.type_arg() {
                v.visit_dart_type(t);
            }
            for elem in n.elements() {
                v.visit_collection_element(elem);
            }
        }
        Expr::Map(n) => {
            for t in n.type_args() {
                v.visit_dart_type(t);
            }
            for element in n.elements() {
                v.visit_map_element(element);
            }
        }
        Expr::Record(n) => {
            for field in n.fields() {
                v.visit_record_field(field);
            }
        }
        Expr::FuncExpr(n) => {
            walk_type_params(v, n.type_params());
            v.visit_formal_param_list(n.params());
            v.visit_function_body(n.body());
        }
        Expr::New(n) => {
            v.visit_dart_type(n.dart_type());
            if let Some(name) = n.constructor_name() {
                v.visit_name(name);
            }
            v.visit_arg_list(n.args());
        }
        Expr::GenericInstantiation(n) => {
            v.visit_expr(n.target());
            for t in n.type_args() {
                v.visit_dart_type(t);
            }
        }
        Expr::Await(n) => v.visit_expr(n.expr()),
        Expr::Throw(n) => v.visit_expr(n.expr()),
        Expr::NullAssert(n) => v.visit_expr(n.operand()),
        Expr::Switch(n) => {
            v.visit_expr(n.subject());
            for arm in n.arms() {
                v.visit_switch_expr_arm(arm);
            }
        }
    }
}

pub fn walk_pattern<'a, V: Visitor<'a>>(v: &mut V, node: Pattern<'a>) {
    match node {
        Pattern::Wildcard(n) => {
            let type_ = n.type_();
            if let Some(t) = type_ {
                v.visit_dart_type(t);
            }
        }
        Pattern::Variable(n) => {
            let type_ = n.type_();
            let name = n.name();
            if let Some(t) = type_ {
                v.visit_dart_type(t);
            }
            v.visit_identifier(name);
        }
        Pattern::Literal(x) => v.visit_literal_pattern(x),
        Pattern::Const(x) => v.visit_const_pattern(x),
        Pattern::Error(_) => {}
        Pattern::List(x) => v.visit_list_pattern(x),
        Pattern::Record(x) => v.visit_record_pattern(x),
        Pattern::Map(x) => v.visit_map_pattern(x),
        Pattern::Object(x) => v.visit_object_pattern(x),
        Pattern::LogicalAnd(n) => {
            let left = n.left();
            let right = n.right();
            v.visit_pattern(left);
            v.visit_pattern(right);
        }
        Pattern::LogicalOr(n) => {
            let left = n.left();
            let right = n.right();
            v.visit_pattern(left);
            v.visit_pattern(right);
        }
        Pattern::Relational(n) => {
            let value = n.value();
            v.visit_expr(value);
        }
        Pattern::Cast(n) => {
            let inner = n.inner();
            let cast_type = n.cast_type();
            v.visit_pattern(inner);
            v.visit_dart_type(cast_type);
        }
        Pattern::NullCheck(n) => {
            let inner = n.inner();
            v.visit_pattern(inner)
        }
        Pattern::NullAssert(n) => {
            let inner = n.inner();
            v.visit_pattern(inner)
        }
        Pattern::ParenPattern(n) => {
            let inner = n.inner();
            v.visit_pattern(inner);
        }
    }
}
pub fn walk_formal_param_annotations<'a, V: Visitor<'a>>(v: &mut V, node: FormalParam<'a>) {
    for annotation in node.annotations() {
        v.visit_annotation(annotation);
    }
    walk_type_params(v, node.type_params());
    if let Some(params) = node.function_params() {
        for param in params.params() {
            walk_formal_param_annotations(v, param);
        }
    }
}

pub fn walk_formal_param<'a, V: Visitor<'a>>(v: &mut V, node: FormalParam<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    walk_type_params(v, node.type_params());
    if let Some(t) = node.param_type() {
        v.visit_dart_type(t);
    }
    v.visit_identifier(node.name());
    if let Some(params) = node.function_params() {
        v.visit_formal_param_list(params);
    }
    if let Some(def) = node.default_value() {
        v.visit_expr(def);
    }
}
pub fn walk_block<'a, V: Visitor<'a>>(v: &mut V, node: Block<'a>) {
    for stmt in node.stmts() {
        v.visit_stmt(stmt);
    }
}

pub fn walk_switch_case<'a, V: Visitor<'a>>(v: &mut V, node: SwitchCase<'a>) {
    for label in node.labels() {
        v.visit_identifier(label);
    }
    for kind in node.cases() {
        v.visit_switch_case_kind(kind);
    }
    for stmt in node.body() {
        v.visit_stmt(stmt);
    }
}

pub fn walk_catch_clause<'a, V: Visitor<'a>>(v: &mut V, node: CatchClause<'a>) {
    if let Some(t) = node.exception_type() {
        v.visit_dart_type(t);
    }
    if let Some(name) = node.exception_var() {
        v.visit_identifier(name);
    }
    if let Some(name) = node.stack_trace_var() {
        v.visit_identifier(name);
    }
    v.visit_block(node.body());
}

pub fn walk_switch_expr_arm<'a, V: Visitor<'a>>(v: &mut V, node: SwitchExprArm<'a>) {
    v.visit_pattern(node.pattern());
    if let Some(guard) = node.guard() {
        v.visit_expr(guard);
    }
    v.visit_expr(node.body());
}
pub fn walk_function_body<'a, V: Visitor<'a>>(v: &mut V, node: FunctionBody<'a>) {
    match node {
        FunctionBody::Block(b) => v.visit_block(b),
        FunctionBody::Arrow(body) => v.visit_expr(body.expr()),
        FunctionBody::Native(body) => {
            if let Some(literal) = body.literal() {
                walk_native_string(v, literal);
            }
        }
    }
}

pub fn walk_arg_list<'a, V: Visitor<'a>>(v: &mut V, node: ArgList<'a>) {
    for arg in node.positional() {
        v.visit_expr(arg);
    }
    for arg in node.named() {
        v.visit_named_arg(arg);
    }
}

pub fn walk_cascade_section<'a, V: Visitor<'a>>(v: &mut V, section: CascadeSection<'a>) {
    for op in section.ops() {
        v.visit_cascade_op(op);
    }
}

pub fn walk_collection_element<'a, V: Visitor<'a>>(v: &mut V, elem: CollectionElement<'a>) {
    match elem {
        CollectionElement::Expr(e) => v.visit_expr(e),
        CollectionElement::NullAware(n) => {
            let expr = n.expr();
            v.visit_expr(expr);
        }
        CollectionElement::Spread(n) => {
            let expr = n.expr();
            v.visit_expr(expr);
        }
        CollectionElement::If(n) => {
            let condition = n.condition();
            let then_elem = n.then_elem();
            let else_elem = n.else_elem();
            v.visit_if_condition(condition);
            v.visit_collection_element(then_elem);
            if let Some(else_e) = else_elem {
                v.visit_collection_element(else_e);
            }
        }
        CollectionElement::For(n) => {
            let header = n.header();
            let element = n.element();
            v.visit_for_header(header);
            v.visit_collection_element(element);
        }
    }
}

pub fn walk_local_var<'a, V: Visitor<'a>>(v: &mut V, node: LocalVarDecl<'a>) {
    for ann in node.annotations() {
        v.visit_annotation(ann);
    }
    if let Some(t) = node.var_type() {
        v.visit_dart_type(t);
    }
    for decl in node.declarators() {
        v.visit_var_declarator(decl);
    }
}

pub fn walk_for_init<'a, V: Visitor<'a>>(v: &mut V, init: ForInit<'a>) {
    match init {
        ForInit::VarDecl(d) => v.visit_local_var(d),
        ForInit::ForIn(n) => {
            let annotations = n.annotations();
            let var_type = n.var_type();
            let name = n.name();
            let iterable = n.iterable();
            for annotation in annotations {
                v.visit_annotation(annotation);
            }
            if let Some(t) = var_type {
                v.visit_dart_type(t);
            }
            v.visit_identifier(name);
            v.visit_expr(iterable);
        }
        ForInit::PatternForIn(n) => {
            let annotations = n.annotations();
            let pattern = n.pattern();
            let iterable = n.iterable();
            for annotation in annotations {
                v.visit_annotation(annotation);
            }
            v.visit_pattern(pattern);
            v.visit_expr(iterable);
        }
        ForInit::Exprs(exprs) => {
            for e in exprs.exprs() {
                v.visit_expr(e);
            }
        }
    }
}

pub fn walk_for_header<'a, V: Visitor<'a>>(v: &mut V, header: ForHeader<'a>) {
    if let Some(init) = header.init() {
        v.visit_for_init(init);
    }
    if let Some(condition) = header.condition() {
        v.visit_expr(condition);
    }
    for update in header.updates() {
        v.visit_expr(update);
    }
}

pub fn walk_map_element<'a, V: Visitor<'a>>(v: &mut V, element: MapElement<'a>) {
    match element {
        MapElement::Entry(entry) => v.visit_map_entry(entry),
        MapElement::Spread(n) => {
            let expr = n.expr();
            v.visit_expr(expr);
        }
        MapElement::If(n) => {
            let condition = n.condition();
            let then_entry = n.then_entry();
            let else_entry = n.else_entry();
            v.visit_if_condition(condition);
            v.visit_map_element(then_entry);
            if let Some(else_e) = else_entry {
                v.visit_map_element(else_e);
            }
        }
        MapElement::For(n) => {
            let header = n.header();
            let entry = n.entry();
            v.visit_for_header(header);
            v.visit_map_element(entry);
        }
    }
}
pub fn walk_enum_variant<'a, V: Visitor<'a>>(v: &mut V, node: EnumVariant<'a>) {
    for annotation in node.annotations() {
        v.visit_annotation(annotation);
    }
    v.visit_identifier(node.name());
    for ty in node.type_args() {
        v.visit_dart_type(ty);
    }
    if let Some(name) = node.constructor_name() {
        v.visit_name(name);
    }
    if let Some(args) = node.args() {
        v.visit_arg_list(args);
    }
}

pub fn walk_var_declarator<'a, V: Visitor<'a>>(v: &mut V, node: VarDeclarator<'a>) {
    v.visit_identifier(node.name());
    if let Some(init) = node.initializer() {
        v.visit_expr(init);
    }
}

pub fn walk_redirected_constructor<'a, V: Visitor<'a>>(v: &mut V, node: RedirectedConstructor<'a>) {
    v.visit_dart_type(node.type_());
    if let Some(name) = node.constructor_name() {
        v.visit_identifier(name);
    }
}

pub fn walk_type_param<'a, V: Visitor<'a>>(v: &mut V, node: TypeParam<'a>) {
    for annotation in node.annotations() {
        v.visit_annotation(annotation);
    }
    v.visit_name(node.name());
    if let Some(bound) = node.bound() {
        v.visit_dart_type(bound);
    }
}

pub fn walk_formal_param_list<'a, V: Visitor<'a>>(v: &mut V, node: FormalParamList<'a>) {
    for param in node.params() {
        v.visit_formal_param(param);
    }
}

pub fn walk_named_type<'a, V: Visitor<'a>>(v: &mut V, node: NamedType<'a>) {
    for segment in node.segments() {
        v.visit_name(segment);
    }
    for arg in node.type_args() {
        v.visit_dart_type(arg);
    }
}

pub fn walk_function_type<'a, V: Visitor<'a>>(v: &mut V, node: FunctionType<'a>) {
    walk_type_params(v, node.type_params());
    if let Some(ret) = node.return_type() {
        v.visit_dart_type(ret);
    }
    for param in node.params() {
        v.visit_function_type_param(param);
    }
}

pub fn walk_function_type_param<'a, V: Visitor<'a>>(v: &mut V, node: FunctionTypeParam<'a>) {
    for annotation in node.annotations() {
        v.visit_annotation(annotation);
    }
    v.visit_dart_type(node.param_type());
    if let Some(name) = node.name() {
        v.visit_name(name);
    }
}

pub fn walk_record_type<'a, V: Visitor<'a>>(v: &mut V, node: RecordType<'a>) {
    for ty in node.positional() {
        v.visit_dart_type(ty);
    }
    for name in node.positional_names().flatten() {
        v.visit_name(name);
    }
    for field in node.named() {
        v.visit_named_record_field(field);
    }
}

pub fn walk_named_record_field<'a, V: Visitor<'a>>(v: &mut V, node: NamedRecordField<'a>) {
    v.visit_dart_type(node.field_type());
    v.visit_name(node.name());
}

pub fn walk_if_condition<'a, V: Visitor<'a>>(v: &mut V, node: IfCondition<'a>) {
    match node {
        IfCondition::Expr(expr) => v.visit_expr(expr),
        IfCondition::Case(condition) => {
            v.visit_expr(condition.expr());
            v.visit_pattern(condition.pattern());
            if let Some(guard) = condition.guard() {
                v.visit_expr(guard);
            }
        }
    }
}

pub fn walk_if_stmt<'a, V: Visitor<'a>>(v: &mut V, node: IfStmt<'a>) {
    v.visit_if_condition(node.condition());
    v.visit_stmt(node.then_branch());
    if let Some(branch) = node.else_branch() {
        v.visit_stmt(branch);
    }
}

pub fn walk_for_stmt<'a, V: Visitor<'a>>(v: &mut V, node: ForStmt<'a>) {
    v.visit_for_header(node.header());
    v.visit_stmt(node.body());
}

pub fn walk_while_stmt<'a, V: Visitor<'a>>(v: &mut V, node: WhileStmt<'a>) {
    v.visit_expr(node.condition());
    v.visit_stmt(node.body());
}

pub fn walk_do_while_stmt<'a, V: Visitor<'a>>(v: &mut V, node: DoWhileStmt<'a>) {
    v.visit_stmt(node.body());
    v.visit_expr(node.condition());
}

pub fn walk_switch_stmt<'a, V: Visitor<'a>>(v: &mut V, node: SwitchStmt<'a>) {
    v.visit_expr(node.subject());
    for case in node.cases() {
        v.visit_switch_case(case);
    }
}

pub fn walk_try_catch_stmt<'a, V: Visitor<'a>>(v: &mut V, node: TryCatchStmt<'a>) {
    v.visit_block(node.body());
    for catch in node.catches() {
        v.visit_catch_clause(catch);
    }
    if let Some(finally) = node.finally() {
        v.visit_block(finally);
    }
}

pub fn walk_return_stmt<'a, V: Visitor<'a>>(v: &mut V, node: ReturnStmt<'a>) {
    if let Some(value) = node.value() {
        v.visit_expr(value);
    }
}

pub fn walk_throw_stmt<'a, V: Visitor<'a>>(v: &mut V, node: ThrowStmt<'a>) {
    v.visit_expr(node.value());
}

pub fn walk_pattern_declaration<'a, V: Visitor<'a>>(v: &mut V, node: PatternDeclaration<'a>) {
    for annotation in node.annotations() {
        v.visit_annotation(annotation);
    }
    v.visit_pattern(node.pattern());
    v.visit_expr(node.init());
}

pub fn walk_pattern_assign_stmt<'a, V: Visitor<'a>>(v: &mut V, node: PatternAssignStmt<'a>) {
    v.visit_pattern(node.pattern());
    v.visit_expr(node.value());
}

pub fn walk_labeled_stmt<'a, V: Visitor<'a>>(v: &mut V, node: LabeledStmt<'a>) {
    v.visit_identifier(node.label());
    v.visit_stmt(node.stmt());
}

pub fn walk_local_func_decl<'a, V: Visitor<'a>>(v: &mut V, node: LocalFuncDecl<'a>) {
    for annotation in node.annotations() {
        v.visit_annotation(annotation);
    }
    if let Some(ret) = node.return_type() {
        v.visit_dart_type(ret);
    }
    v.visit_identifier(node.name());
    walk_type_params(v, node.type_params());
    v.visit_formal_param_list(node.params());
    v.visit_function_body(node.body());
}

pub fn walk_assert_stmt<'a, V: Visitor<'a>>(v: &mut V, node: AssertStmt<'a>) {
    v.visit_expr(node.condition());
    if let Some(message) = node.message() {
        v.visit_expr(message);
    }
}

pub fn walk_yield_stmt<'a, V: Visitor<'a>>(v: &mut V, node: YieldStmt<'a>) {
    v.visit_expr(node.value());
}

pub fn walk_expr_stmt<'a, V: Visitor<'a>>(v: &mut V, node: ExprStmt<'a>) {
    v.visit_expr(node.expr());
}

pub fn walk_switch_case_kind<'a, V: Visitor<'a>>(v: &mut V, node: SwitchCaseKind<'a>) {
    match node {
        SwitchCaseKind::Pattern(label) => {
            v.visit_pattern(label.pattern());
            if let Some(guard) = label.guard() {
                v.visit_expr(guard);
            }
        }
        SwitchCaseKind::Expr(label) => v.visit_expr(label.expr()),
        SwitchCaseKind::Default(_) => {}
    }
}

pub fn walk_cascade_op<'a, V: Visitor<'a>>(v: &mut V, node: CascadeOp<'a>) {
    match node {
        CascadeOp::Field(n) => {
            let name = n.name();
            v.visit_identifier(name);
        }
        CascadeOp::Index(n) => {
            let index = n.index();
            v.visit_expr(index);
        }
        CascadeOp::Call(call) => {
            v.visit_identifier(call.name());
            for ty in call.type_args() {
                v.visit_dart_type(ty);
            }
            v.visit_arg_list(call.args());
        }
        CascadeOp::Invoke(call) => {
            for ty in call.type_args() {
                v.visit_dart_type(ty);
            }
            v.visit_arg_list(call.args());
        }
        CascadeOp::Assign(assign) => {
            v.visit_expr(assign.target());
            v.visit_expr(assign.value());
        }
        CascadeOp::IndexAssign(assign) => {
            v.visit_expr(assign.target());
            v.visit_expr(assign.value());
        }
        CascadeOp::NullAssert(_) => {}
    }
}

pub fn walk_map_entry<'a, V: Visitor<'a>>(v: &mut V, node: MapEntry<'a>) {
    v.visit_expr(node.key());
    v.visit_expr(node.value());
}

pub fn walk_record_field<'a, V: Visitor<'a>>(v: &mut V, node: RecordField<'a>) {
    if let Some(name) = node.name() {
        v.visit_name(name);
    }
    v.visit_expr(node.value());
}

pub fn walk_named_arg<'a, V: Visitor<'a>>(v: &mut V, node: NamedArg<'a>) {
    v.visit_name(node.name());
    v.visit_expr(node.value());
}

pub fn walk_string_interpolation<'a, V: Visitor<'a>>(v: &mut V, node: StringInterpolation<'a>) {
    v.visit_expr(node.expr());
}

pub fn walk_literal_pattern<'a, V: Visitor<'a>>(v: &mut V, node: LiteralPattern<'a>) {
    v.visit_literal_pattern_value(node.value());
}

pub fn walk_literal_pattern_value<'a, V: Visitor<'a>>(v: &mut V, node: LiteralPatternValue<'a>) {
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

pub fn walk_const_pattern<'a, V: Visitor<'a>>(v: &mut V, node: ConstPattern<'a>) {
    v.visit_const_pattern_value(node.value());
}

pub fn walk_const_pattern_value<'a, V: Visitor<'a>>(v: &mut V, node: ConstPatternValue<'a>) {
    match node {
        ConstPatternValue::Name(segments) => {
            for segment in segments {
                v.visit_identifier(segment);
            }
        }
        ConstPatternValue::Expr(expr) => v.visit_expr(expr),
    }
}

pub fn walk_list_pattern<'a, V: Visitor<'a>>(v: &mut V, node: ListPattern<'a>) {
    if let Some(ty) = node.type_arg() {
        v.visit_dart_type(ty);
    }
    for element in node.elements() {
        v.visit_list_pattern_element(element);
    }
}

pub fn walk_list_pattern_element<'a, V: Visitor<'a>>(v: &mut V, node: ListPatternElement<'a>) {
    match node {
        ListPatternElement::Pattern(pattern) => v.visit_pattern(pattern),
        ListPatternElement::Rest(rest) => {
            if let Some(pattern) = rest.pattern() {
                v.visit_pattern(pattern);
            }
        }
    }
}

pub fn walk_record_pattern<'a, V: Visitor<'a>>(v: &mut V, node: RecordPattern<'a>) {
    for field in node.fields() {
        v.visit_record_pattern_field(field);
    }
}

pub fn walk_record_pattern_field<'a, V: Visitor<'a>>(v: &mut V, node: RecordPatternField<'a>) {
    if let Some(name) = node.source_name() {
        v.visit_name(name);
    }
    v.visit_pattern(node.pattern());
}

pub fn walk_map_pattern<'a, V: Visitor<'a>>(v: &mut V, node: MapPattern<'a>) {
    for ty in node.type_args() {
        v.visit_dart_type(ty);
    }
    for entry in node.entries() {
        v.visit_map_pattern_entry(entry);
    }
}

pub fn walk_map_pattern_entry<'a, V: Visitor<'a>>(v: &mut V, node: MapPatternEntry<'a>) {
    v.visit_expr(node.key());
    v.visit_pattern(node.pattern());
}

pub fn walk_object_pattern<'a, V: Visitor<'a>>(v: &mut V, node: ObjectPattern<'a>) {
    v.visit_dart_type(node.type_());
    for field in node.fields() {
        v.visit_object_pattern_field(field);
    }
}

pub fn walk_object_pattern_field<'a, V: Visitor<'a>>(v: &mut V, node: ObjectPatternField<'a>) {
    if let Some(name) = node.source_name() {
        v.visit_name(name);
    }
    if let Some(pattern) = node.pattern() {
        v.visit_pattern(pattern);
    }
}

pub fn walk_function_kind<'a, V: Visitor<'a>>(v: &mut V, node: FunctionKind<'a>) {
    match node {
        FunctionKind::Function(params) | FunctionKind::Setter(params) => {
            v.visit_formal_param_list(params);
        }
        FunctionKind::Getter => {}
    }
}

pub fn walk_break_stmt<'a, V: Visitor<'a>>(v: &mut V, node: BreakStmt<'a>) {
    if let Some(label) = node.label() {
        v.visit_name(label);
    }
}

pub fn walk_continue_stmt<'a, V: Visitor<'a>>(v: &mut V, node: ContinueStmt<'a>) {
    if let Some(label) = node.label() {
        v.visit_name(label);
    }
}
pub fn walk_string_lit<'a, V: Visitor<'a>>(v: &mut V, node: StringLitNode<'a>) {
    for interpolation in node.interpolations() {
        v.visit_string_interpolation(interpolation);
    }
}
fn walk_source_string<'a, V: Visitor<'a>>(v: &mut V, node: StringLitNode<'a>) {
    v.visit_string_lit(node);
    walk_string_lit(v, node);
}
fn walk_native_string<'a, V: Visitor<'a>>(v: &mut V, node: StringLitNode<'a>) {
    v.visit_native_string(node);
    walk_string_lit(v, node);
}

pub fn for_each_expr<'a>(root: Expr<'a>, f: &mut impl FnMut(Expr<'a>)) {
    struct Expressions<'f, F>(&'f mut F);
    impl<'a, F: FnMut(Expr<'a>)> Visitor<'a> for Expressions<'_, F> {
        fn visit_expr(&mut self, node: Expr<'a>) {
            (self.0)(node);
            walk_expr(self, node);
        }
    }
    Expressions(f).visit_expr(root);
}

pub fn for_each_stmt_in_program<'a>(program: Program<'a>, f: impl FnMut(Stmt<'a>)) {
    struct Statements<F>(F);
    impl<'a, F: FnMut(Stmt<'a>)> Visitor<'a> for Statements<F> {
        fn visit_stmt(&mut self, node: Stmt<'a>) {
            (self.0)(node);
            walk_stmt(self, node);
        }
    }
    Statements(f).visit_program(program);
}

pub fn bound_names(pattern: Pattern<'_>) -> Vec<Identifier<'_>> {
    let mut names = Vec::new();
    collect_bound_names(pattern, &mut names);
    names
}

fn collect_bound_names<'a>(pattern: Pattern<'a>, names: &mut Vec<Identifier<'a>>) {
    match pattern {
        Pattern::Variable(node) => names.push(node.name()),
        Pattern::List(node) => {
            for element in node.elements() {
                match element {
                    ListPatternElement::Pattern(pattern) => collect_bound_names(pattern, names),
                    ListPatternElement::Rest(rest) => {
                        if let Some(pattern) = rest.pattern() {
                            collect_bound_names(pattern, names);
                        }
                    }
                }
            }
        }
        Pattern::Record(node) => {
            for field in node.fields() {
                collect_bound_names(field.pattern(), names);
            }
        }
        Pattern::Map(node) => {
            for entry in node.entries() {
                collect_bound_names(entry.pattern(), names);
            }
        }
        Pattern::Object(node) => {
            for field in node.fields() {
                if let Some(pattern) = field.pattern() {
                    collect_bound_names(pattern, names);
                }
            }
        }
        Pattern::LogicalAnd(node) => {
            collect_bound_names(node.left(), names);
            collect_bound_names(node.right(), names);
        }
        Pattern::LogicalOr(node) => {
            collect_bound_names(node.left(), names);
            collect_bound_names(node.right(), names);
        }
        Pattern::Cast(node) => collect_bound_names(node.inner(), names),
        Pattern::NullCheck(node) => collect_bound_names(node.inner(), names),
        Pattern::NullAssert(node) => collect_bound_names(node.inner(), names),
        Pattern::ParenPattern(node) => collect_bound_names(node.inner(), names),
        Pattern::Wildcard(_)
        | Pattern::Literal(_)
        | Pattern::Const(_)
        | Pattern::Relational(_)
        | Pattern::Error(_) => {}
    }
}
