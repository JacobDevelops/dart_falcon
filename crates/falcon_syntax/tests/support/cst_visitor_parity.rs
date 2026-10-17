use falcon_syntax::{ast, cst, visitor};

type MemberHeader = (Option<ast::Span>, Option<(ast::Span, ast::Span)>);

#[derive(Default)]
struct Trace<'s> {
    calls: Vec<(&'static str, ast::Span)>,
    parent: ast::Span,
    source: &'s str,
    constructors: Vec<(ast::Span, ast::Span, bool, bool, bool, bool)>,
    member_headers: Vec<MemberHeader>,
}

macro_rules! ast_span {
    ($this:ident, $node:ident, field) => {
        $node.span
    };
    ($this:ident, $node:ident, method) => {
        *$node.span()
    };
    ($this:ident, $node:ident, parent) => {
        $this.parent
    };
}

macro_rules! cst_span {
    ($this:ident, $node:ident, field) => {
        $node.span()
    };
    ($this:ident, $node:ident, method) => {
        $node.span()
    };
    ($this:ident, $node:ident, parent) => {
        $this.parent
    };
}

macro_rules! ast_member {
    ($this:ident, $node:ident, visit_class_member) => {
        $this.member_headers.push((
            visitor::declared_member_name($node, $this.source),
            visitor::declared_member_body($node).map(|(body, span)| (*body.span(), span)),
        ));
    };
    ($this:ident, $node:ident, $method:ident) => {};
}

macro_rules! cst_member {
    ($this:ident, $node:ident, visit_class_member) => {
        $this.member_headers.push((
            cst::visit::declared_member_name($node, $this.source),
            cst::visit::declared_member_body($node).map(|(body, span)| (body.span(), span)),
        ));
    };
    ($this:ident, $node:ident, $method:ident) => {};
}

macro_rules! record_hooks {
    ($(($method:ident, $ty:ident, $span:ident $(, $walk:ident)?)),* $(,)?) => {
        impl visitor::Visitor for Trace<'_> {
            $(fn $method(&mut self, node: &ast::$ty) {
                let parent = self.parent;
                self.parent = ast_span!(self, node, $span);
                self.calls.push((stringify!($method), self.parent));
                ast_member!(self, node, $method);
                $(visitor::$walk(self, node);)?
                self.parent = parent;
            })*
            fn visit_constructor(&mut self, node: ast::ConstructorRef<'_>) {
                self.calls.push(("visit_constructor", node.span));
                self.constructors.push((node.span, node.name_span, node.is_primary, node.is_const, node.is_factory, node.is_external));
            }
        }
        impl<'a> cst::visit::Visitor<'a> for Trace<'_> {
            $(fn $method(&mut self, node: cst::$ty<'a>) {
                let parent = self.parent;
                self.parent = cst_span!(self, node, $span);
                self.calls.push((stringify!($method), self.parent));
                cst_member!(self, node, $method);
                $(cst::visit::$walk(self, node);)?
                self.parent = parent;
            })*
            fn visit_constructor(&mut self, node: cst::ConstructorRef<'a>) {
                self.calls.push(("visit_constructor", node.span));
                self.constructors.push((node.span, node.name_span, node.is_primary, node.is_const, node.is_factory, node.is_external));
            }
        }
    };
}

record_hooks!(
    (visit_program, Program, field, walk_program),
    (
        visit_type_alias_decl,
        TypeAliasDecl,
        field,
        walk_type_alias_decl
    ),
    (visit_import, ImportDirective, field, walk_import),
    (visit_export, ExportDirective, field, walk_export),
    (
        visit_top_level_decl,
        TopLevelDecl,
        method,
        walk_top_level_decl
    ),
    (visit_class_decl, ClassDecl, field, walk_class_decl),
    (visit_mixin_decl, MixinDecl, field, walk_mixin_decl),
    (visit_enum_decl, EnumDecl, field, walk_enum_decl),
    (
        visit_extension_decl,
        ExtensionDecl,
        field,
        walk_extension_decl
    ),
    (visit_function_decl, FunctionDecl, field, walk_function_decl),
    (visit_class_member, ClassMember, method, walk_class_member),
    (visit_field_decl, FieldDecl, field, walk_field_decl),
    (
        visit_constructor_decl,
        ConstructorDecl,
        field,
        walk_constructor_decl
    ),
    (visit_method_decl, MethodDecl, field, walk_method_decl),
    (visit_getter_decl, GetterDecl, field, walk_getter_decl),
    (visit_setter_decl, SetterDecl, field, walk_setter_decl),
    (visit_dart_type, DartType, method, walk_dart_type),
    (visit_stmt, Stmt, method, walk_stmt),
    (visit_block, Block, field, walk_block),
    (
        visit_function_body,
        FunctionBody,
        method,
        walk_function_body
    ),
    (visit_switch_case, SwitchCase, field, walk_switch_case),
    (visit_catch_clause, CatchClause, field, walk_catch_clause),
    (visit_local_var, LocalVarDecl, field, walk_local_var),
    (visit_for_header, ForHeader, parent, walk_for_header),
    (visit_expr, Expr, method, walk_expr),
    (
        visit_collection_element,
        CollectionElement,
        parent,
        walk_collection_element
    ),
    (visit_map_element, MapElement, parent, walk_map_element),
    (
        visit_switch_expr_arm,
        SwitchExprArm,
        field,
        walk_switch_expr_arm
    ),
    (visit_pattern, Pattern, method, walk_pattern),
    (visit_annotation, Annotation, field, walk_annotation),
    (visit_identifier, Identifier, field),
    (visit_string_lit, StringLitNode, field),
    (visit_formal_param, FormalParam, field, walk_formal_param),
    (
        visit_primary_constructor,
        PrimaryConstructor,
        field,
        walk_primary_constructor
    ),
    (
        visit_primary_constructor_body,
        PrimaryConstructorBody,
        field,
        walk_primary_constructor_body
    ),
    (
        visit_library_directive,
        LibraryDirective,
        field,
        walk_library_directive
    ),
    (
        visit_part_of_directive,
        PartOfDirective,
        field,
        walk_part_of_directive
    ),
    (
        visit_part_directive,
        PartDirective,
        field,
        walk_part_directive
    ),
    (
        visit_configurable_uri,
        ConfigurableUri,
        field,
        walk_configurable_uri
    ),
    (
        visit_import_combinator,
        ImportCombinator,
        parent,
        walk_import_combinator
    ),
    (
        visit_class_type_alias,
        ClassTypeAliasDecl,
        field,
        walk_class_type_alias
    ),
    (visit_mixin_class, MixinClassDecl, field, walk_mixin_class),
    (
        visit_extension_type,
        ExtensionTypeDecl,
        field,
        walk_extension_type
    ),
    (
        visit_top_level_var,
        TopLevelVarDecl,
        field,
        walk_top_level_var
    ),
    (visit_operator_decl, OperatorDecl, field, walk_operator_decl),
    (
        visit_constructor_initializer,
        ConstructorInitializer,
        parent,
        walk_constructor_initializer
    ),
    (visit_arg_list, ArgList, field, walk_arg_list),
    (
        visit_cascade_section,
        CascadeSection,
        field,
        walk_cascade_section
    ),
    (visit_for_init, ForInit, parent, walk_for_init),
    (visit_enum_variant, EnumVariant, field, walk_enum_variant),
    (
        visit_var_declarator,
        VarDeclarator,
        field,
        walk_var_declarator
    ),
    (
        visit_redirected_constructor,
        RedirectedConstructor,
        field,
        walk_redirected_constructor
    ),
    (visit_type_param, TypeParam, field, walk_type_param),
    (
        visit_formal_param_list,
        FormalParamList,
        field,
        walk_formal_param_list
    ),
    (visit_named_type, NamedType, field, walk_named_type),
    (visit_function_type, FunctionType, field, walk_function_type),
    (
        visit_function_type_param,
        FunctionTypeParam,
        parent,
        walk_function_type_param
    ),
    (visit_record_type, RecordType, field, walk_record_type),
    (
        visit_named_record_field,
        NamedRecordField,
        parent,
        walk_named_record_field
    ),
    (visit_if_condition, IfCondition, parent, walk_if_condition),
    (visit_if_stmt, IfStmt, field, walk_if_stmt),
    (visit_for_stmt, ForStmt, field, walk_for_stmt),
    (visit_while_stmt, WhileStmt, field, walk_while_stmt),
    (visit_do_while_stmt, DoWhileStmt, field, walk_do_while_stmt),
    (visit_switch_stmt, SwitchStmt, field, walk_switch_stmt),
    (
        visit_try_catch_stmt,
        TryCatchStmt,
        field,
        walk_try_catch_stmt
    ),
    (visit_return_stmt, ReturnStmt, field, walk_return_stmt),
    (visit_throw_stmt, ThrowStmt, field, walk_throw_stmt),
    (visit_break_stmt, BreakStmt, field, walk_break_stmt),
    (visit_continue_stmt, ContinueStmt, field, walk_continue_stmt),
    (
        visit_pattern_declaration,
        PatternDeclaration,
        field,
        walk_pattern_declaration
    ),
    (
        visit_pattern_assign_stmt,
        PatternAssignStmt,
        field,
        walk_pattern_assign_stmt
    ),
    (visit_labeled_stmt, LabeledStmt, field, walk_labeled_stmt),
    (
        visit_local_func_decl,
        LocalFuncDecl,
        field,
        walk_local_func_decl
    ),
    (visit_assert_stmt, AssertStmt, field, walk_assert_stmt),
    (visit_yield_stmt, YieldStmt, field, walk_yield_stmt),
    (visit_expr_stmt, ExprStmt, field, walk_expr_stmt),
    (visit_error_node, ErrorNode, field),
    (
        visit_switch_case_kind,
        SwitchCaseKind,
        parent,
        walk_switch_case_kind
    ),
    (visit_cascade_op, CascadeOp, parent, walk_cascade_op),
    (visit_map_entry, MapEntry, field, walk_map_entry),
    (visit_record_field, RecordField, field, walk_record_field),
    (visit_named_arg, NamedArg, field, walk_named_arg),
    (
        visit_string_interpolation,
        StringInterpolation,
        field,
        walk_string_interpolation
    ),
    (
        visit_literal_pattern,
        LiteralPattern,
        field,
        walk_literal_pattern
    ),
    (
        visit_literal_pattern_value,
        LiteralPatternValue,
        parent,
        walk_literal_pattern_value
    ),
    (visit_const_pattern, ConstPattern, field, walk_const_pattern),
    (
        visit_const_pattern_value,
        ConstPatternValue,
        parent,
        walk_const_pattern_value
    ),
    (visit_list_pattern, ListPattern, field, walk_list_pattern),
    (
        visit_list_pattern_element,
        ListPatternElement,
        parent,
        walk_list_pattern_element
    ),
    (
        visit_record_pattern,
        RecordPattern,
        field,
        walk_record_pattern
    ),
    (
        visit_record_pattern_field,
        RecordPatternField,
        field,
        walk_record_pattern_field
    ),
    (visit_map_pattern, MapPattern, field, walk_map_pattern),
    (
        visit_map_pattern_entry,
        MapPatternEntry,
        field,
        walk_map_pattern_entry
    ),
    (
        visit_object_pattern,
        ObjectPattern,
        field,
        walk_object_pattern
    ),
    (
        visit_object_pattern_field,
        ObjectPatternField,
        field,
        walk_object_pattern_field
    ),
    (
        visit_function_kind,
        FunctionKind,
        parent,
        walk_function_kind
    ),
    (visit_name, Identifier, field),
    (visit_native_string, StringLitNode, field),
);

pub fn assert_parity(source: &str, parsed: &falcon_dart_parser::parser::ParseResult) {
    let mut old = Trace {
        source,
        ..Trace::default()
    };
    visitor::Visitor::visit_program(&mut old, &parsed.program);
    let mut new = Trace {
        source,
        ..Trace::default()
    };
    cst::visit::Visitor::visit_program(&mut new, parsed.tree.program(source));
    assert_eq!(old.calls, new.calls, "visitor mismatch for {source:?}");
    assert_eq!(
        old.constructors, new.constructors,
        "constructor mismatch for {source:?}"
    );
    assert_eq!(
        old.member_headers, new.member_headers,
        "member header mismatch for {source:?}"
    );
    let mut old_lists = Vec::new();
    visitor::for_each_stmt_list(&parsed.program, &mut |statements, kind| {
        old_lists.push((
            kind == visitor::StatementListKind::SwitchCase,
            statements
                .iter()
                .map(|statement| *statement.span())
                .collect::<Vec<_>>(),
        ));
    });
    let mut new_lists = Vec::new();
    cst::visit::for_each_stmt_list(parsed.tree.program(source), &mut |statements, kind| {
        new_lists.push((
            kind == cst::visit::StatementListKind::SwitchCase,
            statements
                .map(|statement| statement.span())
                .collect::<Vec<_>>(),
        ));
    });
    assert_eq!(
        old_lists, new_lists,
        "statement-list mismatch for {source:?}"
    );
}
