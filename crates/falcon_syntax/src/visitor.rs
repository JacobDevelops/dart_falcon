use crate::ast::*;

/// A visitor that walks the Dart AST.
///
/// Every `visit_*` method defaults to calling the corresponding `walk_*`
/// helper, which recurses into children.  Override individual methods to
/// intercept specific node kinds; call the walk helper from within the
/// override when you want to continue the traversal.
#[allow(unused_variables)]
pub trait Visitor: Sized {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
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

    fn visit_expr(&mut self, node: &Expr) {
        walk_expr(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        walk_pattern(self, node);
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        walk_annotation(self, node);
    }

    fn visit_identifier(&mut self, node: &Identifier) {}

    fn visit_string_lit(&mut self, node: &StringLitNode) {}

    fn visit_formal_param(&mut self, node: &FormalParam) {
        walk_formal_param(self, node);
    }
}

// ── Walk helpers ──────────────────────────────────────────────────────────────

pub fn walk_program<V: Visitor>(v: &mut V, node: &Program) {
    if let Some(ref lib) = node.library_directive {
        walk_library_directive(v, lib);
    }
    if let Some(ref part_of) = node.part_of_directive {
        walk_part_of_directive(v, part_of);
    }
    for part in &node.part_directives {
        walk_part_directive(v, part);
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

fn walk_library_directive<V: Visitor>(v: &mut V, node: &LibraryDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    for seg in &node.name {
        v.visit_identifier(seg);
    }
}

fn walk_part_of_directive<V: Visitor>(v: &mut V, node: &PartOfDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref uri) = node.uri {
        v.visit_string_lit(uri);
    }
    for seg in &node.name {
        v.visit_identifier(seg);
    }
}

fn walk_part_directive<V: Visitor>(v: &mut V, node: &PartDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_string_lit(&node.uri);
}

pub fn walk_import<V: Visitor>(v: &mut V, node: &ImportDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_string_lit(&node.uri);
    for cu in &node.configurable_uris {
        walk_configurable_uri(v, cu);
    }
    if let Some(ref alias) = node.as_name {
        v.visit_identifier(alias);
    }
    for comb in &node.combinators {
        walk_import_combinator(v, comb);
    }
}

pub fn walk_export<V: Visitor>(v: &mut V, node: &ExportDirective) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_string_lit(&node.uri);
    for cu in &node.configurable_uris {
        walk_configurable_uri(v, cu);
    }
    for comb in &node.combinators {
        walk_import_combinator(v, comb);
    }
}

fn walk_configurable_uri<V: Visitor>(v: &mut V, node: &ConfigurableUri) {
    for seg in &node.test {
        v.visit_identifier(seg);
    }
    if let Some(ref value) = node.value {
        v.visit_string_lit(value);
    }
    v.visit_string_lit(&node.uri);
}

fn walk_import_combinator<V: Visitor>(v: &mut V, comb: &ImportCombinator) {
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
        walk_arg_list(v, args);
    }
}

/// Visit the bound type of each type parameter. Type-parameter names are
/// declaration sites and are left unvisited, matching the other declaration
/// walkers.
fn walk_type_params<V: Visitor>(v: &mut V, type_params: &[TypeParam]) {
    for tp in type_params {
        for ann in &tp.annotations {
            v.visit_annotation(ann);
        }
        if let Some(ref bound) = tp.bound {
            v.visit_dart_type(bound);
        }
    }
}

pub fn walk_top_level_decl<V: Visitor>(v: &mut V, node: &TopLevelDecl) {
    match node {
        TopLevelDecl::Class(x) => v.visit_class_decl(x),
        TopLevelDecl::ClassTypeAlias(x) => walk_class_type_alias(v, x),
        TopLevelDecl::Mixin(x) => v.visit_mixin_decl(x),
        TopLevelDecl::MixinClass(x) => walk_mixin_class(v, x),
        TopLevelDecl::Enum(x) => v.visit_enum_decl(x),
        TopLevelDecl::Extension(x) => v.visit_extension_decl(x),
        TopLevelDecl::ExtensionType(x) => walk_extension_type(v, x),
        TopLevelDecl::Function(x) => v.visit_function_decl(x),
        TopLevelDecl::Variable(x) => walk_top_level_var(v, x),
        TopLevelDecl::TypeAlias(x) => walk_type_alias(v, x),
        TopLevelDecl::Error(_) => {}
    }
}

pub fn walk_class_decl<V: Visitor>(v: &mut V, node: &ClassDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
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

fn walk_class_type_alias<V: Visitor>(v: &mut V, node: &ClassTypeAliasDecl) {
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

fn walk_mixin_class<V: Visitor>(v: &mut V, node: &MixinClassDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
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
    for t in &node.with_clause {
        v.visit_dart_type(t);
    }
    for t in &node.implements {
        v.visit_dart_type(t);
    }
    for variant in &node.variants {
        for ann in &variant.annotations {
            v.visit_annotation(ann);
        }
        v.visit_identifier(&variant.name);
        for t in &variant.type_args {
            v.visit_dart_type(t);
        }
        if let Some(ref args) = variant.args {
            walk_arg_list(v, args);
        }
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

fn walk_extension_type<V: Visitor>(v: &mut V, node: &ExtensionTypeDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    if let Some(ref ctor) = node.representation.constructor_name {
        v.visit_identifier(ctor);
    }
    v.visit_dart_type(&node.representation.field_type);
    v.visit_identifier(&node.representation.field_name);
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
    for param in node
        .params
        .positional
        .iter()
        .chain(&node.params.optional_positional)
        .chain(&node.params.named)
    {
        v.visit_formal_param(param);
    }
    if let Some(ref body) = node.body {
        walk_function_body(v, body);
    }
}

fn walk_top_level_var<V: Visitor>(v: &mut V, node: &TopLevelVarDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref t) = node.var_type {
        v.visit_dart_type(t);
    }
    for decl in &node.declarators {
        v.visit_identifier(&decl.name);
        if let Some(ref init) = decl.initializer {
            v.visit_expr(init);
        }
    }
}

fn walk_type_alias<V: Visitor>(v: &mut V, node: &TypeAliasDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    walk_type_params(v, &node.type_params);
    v.visit_dart_type(&node.aliased);
}

pub fn walk_class_member<V: Visitor>(v: &mut V, node: &ClassMember) {
    match node {
        ClassMember::Field(x) => v.visit_field_decl(x),
        ClassMember::Constructor(x) => v.visit_constructor_decl(x),
        ClassMember::Method(x) => v.visit_method_decl(x),
        ClassMember::Getter(x) => v.visit_getter_decl(x),
        ClassMember::Setter(x) => v.visit_setter_decl(x),
        ClassMember::Operator(x) => walk_operator_decl(v, x),
        ClassMember::Error(_) => {}
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
        v.visit_identifier(&decl.name);
        if let Some(ref init) = decl.initializer {
            v.visit_expr(init);
        }
    }
}

pub fn walk_constructor_decl<V: Visitor>(v: &mut V, node: &ConstructorDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    v.visit_identifier(&node.name);
    for param in node
        .params
        .positional
        .iter()
        .chain(&node.params.optional_positional)
        .chain(&node.params.named)
    {
        v.visit_formal_param(param);
    }
    for init in &node.initializers {
        walk_constructor_initializer(v, init);
    }
    if let Some(ref redirect) = node.redirect {
        v.visit_dart_type(&redirect.type_);
        if let Some(ref name) = redirect.constructor_name {
            v.visit_identifier(name);
        }
    }
    if let Some(ref body) = node.body {
        walk_function_body(v, body);
    }
}

fn walk_constructor_initializer<V: Visitor>(v: &mut V, init: &ConstructorInitializer) {
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
            walk_arg_list(v, args);
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
    for param in node
        .params
        .positional
        .iter()
        .chain(&node.params.optional_positional)
        .chain(&node.params.named)
    {
        v.visit_formal_param(param);
    }
    if let Some(ref body) = node.body {
        walk_function_body(v, body);
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
        walk_function_body(v, body);
    }
}

pub fn walk_setter_decl<V: Visitor>(v: &mut V, node: &SetterDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref t) = node.param_type {
        v.visit_dart_type(t);
    }
    v.visit_identifier(&node.name);
    if let Some(ref body) = node.body {
        walk_function_body(v, body);
    }
}

fn walk_operator_decl<V: Visitor>(v: &mut V, node: &OperatorDecl) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref ret) = node.return_type {
        v.visit_dart_type(ret);
    }
    for param in node
        .params
        .positional
        .iter()
        .chain(&node.params.optional_positional)
        .chain(&node.params.named)
    {
        v.visit_formal_param(param);
    }
    if let Some(ref body) = node.body {
        walk_function_body(v, body);
    }
}

pub fn walk_dart_type<V: Visitor>(v: &mut V, node: &DartType) {
    match node {
        DartType::Named(x) => {
            for arg in &x.type_args {
                v.visit_dart_type(arg);
            }
        }
        DartType::Function(x) => {
            walk_type_params(v, &x.type_params);
            if let Some(ref ret) = x.return_type {
                v.visit_dart_type(ret);
            }
            for param in &x.params {
                v.visit_dart_type(&param.param_type);
            }
        }
        DartType::Record(x) => {
            for t in &x.positional {
                v.visit_dart_type(t);
            }
            for f in &x.named {
                v.visit_dart_type(&f.field_type);
            }
        }
        DartType::Void { .. } | DartType::Dynamic { .. } | DartType::Never { .. } => {}
    }
}

pub fn walk_stmt<V: Visitor>(v: &mut V, node: &Stmt) {
    match node {
        Stmt::Block(x) => {
            for s in &x.stmts {
                v.visit_stmt(s);
            }
        }
        Stmt::If(x) => {
            match &x.condition {
                IfCondition::Expr(e) => v.visit_expr(e),
                IfCondition::Case(e, p, guard) => {
                    v.visit_expr(e);
                    v.visit_pattern(p);
                    if let Some(g) = guard {
                        v.visit_expr(g);
                    }
                }
            }
            v.visit_stmt(&x.then_branch);
            if let Some(ref else_b) = x.else_branch {
                v.visit_stmt(else_b);
            }
        }
        Stmt::For(x) => {
            if let Some(ref init) = x.init {
                match init {
                    ForInit::VarDecl(d) => v.visit_stmt(&Stmt::LocalVar(d.clone())),
                    ForInit::ForIn {
                        var_type,
                        name,
                        iterable,
                        ..
                    } => {
                        if let Some(t) = var_type {
                            v.visit_dart_type(t);
                        }
                        v.visit_identifier(name);
                        v.visit_expr(iterable);
                    }
                    ForInit::PatternForIn { pattern, iterable } => {
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
            if let Some(ref cond) = x.condition {
                v.visit_expr(cond);
            }
            for e in &x.update {
                v.visit_expr(e);
            }
            v.visit_stmt(&x.body);
        }
        Stmt::While(x) => {
            v.visit_expr(&x.condition);
            v.visit_stmt(&x.body);
        }
        Stmt::DoWhile(x) => {
            v.visit_stmt(&x.body);
            v.visit_expr(&x.condition);
        }
        Stmt::Switch(x) => {
            v.visit_expr(&x.subject);
            for case in &x.cases {
                for kind in &case.cases {
                    match kind {
                        SwitchCaseKind::Pattern(p, guard) => {
                            v.visit_pattern(p);
                            if let Some(g) = &**guard {
                                v.visit_expr(g);
                            }
                        }
                        SwitchCaseKind::Default => {}
                    }
                }
                for s in &case.body {
                    v.visit_stmt(s);
                }
            }
        }
        Stmt::TryCatch(x) => {
            for s in &x.body.stmts {
                v.visit_stmt(s);
            }
            for catch in &x.catches {
                for s in &catch.body.stmts {
                    v.visit_stmt(s);
                }
            }
            if let Some(ref fin) = x.finally {
                for s in &fin.stmts {
                    v.visit_stmt(s);
                }
            }
        }
        Stmt::Return(x) => {
            if let Some(ref val) = x.value {
                v.visit_expr(val);
            }
        }
        Stmt::Throw(x) => v.visit_expr(&x.value),
        Stmt::LocalVar(x) => {
            if let Some(ref t) = x.var_type {
                v.visit_dart_type(t);
            }
            for decl in &x.declarators {
                v.visit_identifier(&decl.name);
                if let Some(ref init) = decl.initializer {
                    v.visit_expr(init);
                }
            }
        }
        Stmt::PatternDecl(x) => {
            v.visit_pattern(&x.pattern);
            v.visit_expr(&x.init);
        }
        Stmt::PatternAssign(x) => {
            v.visit_pattern(&x.pattern);
            v.visit_expr(&x.value);
        }
        Stmt::Labeled(x) => {
            v.visit_identifier(&x.label);
            v.visit_stmt(&x.stmt);
        }
        Stmt::LocalFunc(x) => {
            if let Some(ref ret) = x.return_type {
                v.visit_dart_type(ret);
            }
            v.visit_identifier(&x.name);
            walk_type_params(v, &x.type_params);
            for param in x
                .params
                .positional
                .iter()
                .chain(&x.params.optional_positional)
                .chain(&x.params.named)
            {
                v.visit_formal_param(param);
            }
            walk_function_body(v, &x.body);
        }
        Stmt::Assert(x) => {
            v.visit_expr(&x.condition);
            if let Some(ref msg) = x.message {
                v.visit_expr(msg);
            }
        }
        Stmt::Yield(x) => v.visit_expr(&x.value),
        Stmt::Expr(x) => v.visit_expr(&x.expr),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Error(_) => {}
    }
}

pub fn walk_expr<V: Visitor>(v: &mut V, node: &Expr) {
    match node {
        Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. }
        | Expr::This { .. }
        | Expr::Super { .. }
        | Expr::DotShorthand { .. }
        | Expr::SymbolLit { .. }
        | Expr::Error { .. } => {}

        Expr::StringLit(x) => {
            v.visit_string_lit(x);
            // Interpolated expressions are real, analyzable sub-expressions.
            for interp in &x.interpolations {
                v.visit_expr(&interp.expr);
            }
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
        Expr::Field { object, .. } => v.visit_expr(object),
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
            walk_arg_list(v, args);
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            v.visit_expr(object);
            for section in sections {
                walk_cascade_section(v, section);
            }
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for elem in elements {
                walk_collection_element(v, elem);
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                v.visit_expr(&entry.key);
                v.visit_expr(&entry.value);
            }
            for element in elements {
                walk_map_element(v, element);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                v.visit_expr(&field.value);
            }
        }
        Expr::FuncExpr {
            type_params,
            params,
            body,
            ..
        } => {
            walk_type_params(v, type_params);
            for param in params
                .positional
                .iter()
                .chain(&params.optional_positional)
                .chain(&params.named)
            {
                v.visit_formal_param(param);
            }
            walk_function_body(v, body);
        }
        Expr::New {
            dart_type, args, ..
        } => {
            v.visit_dart_type(dart_type);
            walk_arg_list(v, args);
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
                v.visit_pattern(&arm.pattern);
                if let Some(ref guard) = arm.guard {
                    v.visit_expr(guard);
                }
                v.visit_expr(&arm.body);
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
        Pattern::Literal(x) => {
            if let LiteralPatternValue::String(s) = &x.value {
                v.visit_string_lit(s);
            }
        }
        Pattern::Const(x) => {
            if let Some(ref e) = x.expr {
                v.visit_expr(e);
            }
        }
        Pattern::Error { .. } => {}
        Pattern::List(x) => {
            for elem in &x.elements {
                match elem {
                    ListPatternElement::Pattern(p) => v.visit_pattern(p),
                    ListPatternElement::Rest(Some(p), _) => v.visit_pattern(p),
                    ListPatternElement::Rest(None, _) => {}
                }
            }
        }
        Pattern::Record(x) => {
            for field in &x.fields {
                v.visit_pattern(&field.pattern);
            }
        }
        Pattern::Map(x) => {
            for entry in &x.entries {
                v.visit_expr(&entry.key);
                v.visit_pattern(&entry.pattern);
            }
        }
        Pattern::Object(x) => {
            v.visit_dart_type(&x.type_);
            for field in &x.fields {
                if let Some(ref p) = field.pattern {
                    v.visit_pattern(p);
                }
            }
        }
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

pub fn walk_formal_param<V: Visitor>(v: &mut V, node: &FormalParam) {
    for ann in &node.annotations {
        v.visit_annotation(ann);
    }
    if let Some(ref t) = node.param_type {
        v.visit_dart_type(t);
    }
    v.visit_identifier(&node.name);
    if let Some(ref params) = node.function_params {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            v.visit_formal_param(param);
        }
    }
    if let Some(ref def) = node.default_value {
        v.visit_expr(def);
    }
}

fn walk_function_body<V: Visitor>(v: &mut V, node: &FunctionBody) {
    match node {
        FunctionBody::Block(b) => {
            for s in &b.stmts {
                v.visit_stmt(s);
            }
        }
        FunctionBody::Arrow(expr, _) => v.visit_expr(expr),
        FunctionBody::Native(_, _) => {}
    }
}

fn walk_arg_list<V: Visitor>(v: &mut V, node: &ArgList) {
    for arg in &node.positional {
        v.visit_expr(arg);
    }
    for arg in &node.named {
        v.visit_expr(&arg.value);
    }
}

fn walk_cascade_section<V: Visitor>(v: &mut V, section: &CascadeSection) {
    match &section.op {
        CascadeOp::Field(ident, _) => v.visit_identifier(ident),
        CascadeOp::Index(expr, _) => v.visit_expr(expr),
        CascadeOp::Call(ident, type_args, args) => {
            v.visit_identifier(ident);
            for t in type_args {
                v.visit_dart_type(t);
            }
            walk_arg_list(v, args);
        }
        CascadeOp::Assign(target, _, value) => {
            v.visit_expr(target);
            v.visit_expr(value);
        }
    }
}

fn walk_collection_element<V: Visitor>(v: &mut V, elem: &CollectionElement) {
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
            match condition {
                IfCondition::Expr(e) => v.visit_expr(e),
                IfCondition::Case(e, p, guard) => {
                    v.visit_expr(e);
                    v.visit_pattern(p);
                    if let Some(g) = guard {
                        v.visit_expr(g);
                    }
                }
            }
            walk_collection_element(v, then_elem);
            if let Some(else_e) = else_elem {
                walk_collection_element(v, else_e);
            }
        }
        CollectionElement::For {
            pattern,
            iterable,
            element,
            ..
        } => {
            if let Some(p) = pattern {
                v.visit_pattern(p);
            }
            v.visit_expr(iterable);
            walk_collection_element(v, element);
        }
        CollectionElement::CFor {
            init,
            condition,
            updates,
            element,
            ..
        } => {
            walk_for_init(v, init);
            if let Some(cond) = condition {
                v.visit_expr(cond);
            }
            for e in updates {
                v.visit_expr(e);
            }
            walk_collection_element(v, element);
        }
    }
}

fn walk_for_init<V: Visitor>(v: &mut V, init: &Option<ForInit>) {
    let Some(init) = init else { return };
    match init {
        ForInit::VarDecl(d) => v.visit_stmt(&Stmt::LocalVar(d.clone())),
        ForInit::ForIn {
            var_type,
            name,
            iterable,
            ..
        } => {
            if let Some(t) = var_type {
                v.visit_dart_type(t);
            }
            v.visit_identifier(name);
            v.visit_expr(iterable);
        }
        ForInit::PatternForIn { pattern, iterable } => {
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

fn walk_map_element<V: Visitor>(v: &mut V, element: &MapElement) {
    match element {
        MapElement::Entry(entry) => {
            v.visit_expr(&entry.key);
            v.visit_expr(&entry.value);
        }
        MapElement::Spread { expr, .. } => v.visit_expr(expr),
        MapElement::If {
            condition,
            then_entry,
            else_entry,
            ..
        } => {
            match condition {
                IfCondition::Expr(e) => v.visit_expr(e),
                IfCondition::Case(e, p, guard) => {
                    v.visit_expr(e);
                    v.visit_pattern(p);
                    if let Some(g) = guard {
                        v.visit_expr(g);
                    }
                }
            }
            walk_map_element(v, then_entry);
            if let Some(else_e) = else_entry {
                walk_map_element(v, else_e);
            }
        }
        MapElement::For {
            pattern,
            iterable,
            entry,
            ..
        } => {
            if let Some(p) = pattern {
                v.visit_pattern(p);
            }
            v.visit_expr(iterable);
            walk_map_element(v, entry);
        }
        MapElement::CFor {
            init,
            condition,
            updates,
            entry,
            ..
        } => {
            walk_for_init(v, init);
            if let Some(cond) = condition {
                v.visit_expr(cond);
            }
            for e in updates {
                v.visit_expr(e);
            }
            walk_map_element(v, entry);
        }
    }
}
