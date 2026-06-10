use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::*;

// ── Visitor pattern test ──────────────────────────────────────────────────────

struct CountingVisitor {
    node_count: usize,
    class_count: usize,
    function_count: usize,
    field_count: usize,
}

impl Visitor for CountingVisitor {
    fn visit_program(&mut self, node: &Program) {
        self.node_count += 1;
        walk_program(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        self.node_count += 1;
        walk_top_level_decl(self, node);
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.class_count += 1;
        self.node_count += 1;
        walk_class_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function_count += 1;
        self.node_count += 1;
        walk_function_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.field_count += 1;
        self.node_count += 1;
        walk_field_decl(self, node);
    }
}

#[test]
fn test_visitor_empty_program() {
    let src = "";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor {
        node_count: 0,
        class_count: 0,
        function_count: 0,
        field_count: 0,
    };
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 0);
    assert_eq!(visitor.function_count, 0);
}

#[test]
fn test_visitor_single_class() {
    let src = "class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor {
        node_count: 0,
        class_count: 0,
        function_count: 0,
        field_count: 0,
    };
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 1);
}

#[test]
fn test_visitor_class_with_fields() {
    let src = "class Foo { int x; String y; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor {
        node_count: 0,
        class_count: 0,
        function_count: 0,
        field_count: 0,
    };
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 1);
    assert_eq!(visitor.field_count, 2);
}

#[test]
fn test_visitor_multiple_declarations() {
    let src = "class Foo {} void bar() {} class Baz {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor {
        node_count: 0,
        class_count: 0,
        function_count: 0,
        field_count: 0,
    };
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 2);
    assert_eq!(visitor.function_count, 1);
}

// ── Span tests ────────────────────────────────────────────────────────────────

#[test]
fn test_span_merge() {
    let span1 = Span::new(0, 10);
    let span2 = Span::new(15, 25);

    let merged = span1.merge(&span2);
    assert_eq!(merged.start, 0);
    assert_eq!(merged.end, 25);
}

#[test]
fn test_span_merge_overlapping() {
    let span1 = Span::new(0, 15);
    let span2 = Span::new(10, 20);

    let merged = span1.merge(&span2);
    assert_eq!(merged.start, 0);
    assert_eq!(merged.end, 20);
}

#[test]
fn test_span_merge_same() {
    let span = Span::new(5, 10);
    let merged = span.merge(&span);

    assert_eq!(merged.start, 5);
    assert_eq!(merged.end, 10);
}

#[test]
fn test_span_from_program() {
    let src = "class Foo {}";
    let (prog, _) = parse(src);

    // Program should have a span covering the entire source
    assert_eq!(prog.span.start, 0);
    assert!(prog.span.end > 0);
}

#[test]
fn test_span_nested_declarations() {
    let src = "class Parent { class Child {} }";
    let (prog, errors) = parse(src);
    // Parse should handle or error gracefully on invalid Dart
    // Class nesting is not valid in Dart, but parser should not panic
    assert!(!prog.declarations.is_empty() || !errors.is_empty());
}

// ── Type span tests ──────────────────────────────────────────────────────────

#[test]
fn test_type_named_span() {
    let src = "int x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    assert!(v.var_type.is_some());
    let span = v.var_type.as_ref().unwrap().span();
    assert!(span.end > span.start);
}

#[test]
fn test_type_generic_span() {
    let src = "List<int> list;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    assert!(v.var_type.is_some());
    let span = v.var_type.as_ref().unwrap().span();
    assert!(span.end > span.start);
}

// ── Identifier tests ──────────────────────────────────────────────────────────

#[test]
fn test_identifier_basic() {
    let src = "class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.name.name, "Foo");
    assert!(cls.name.span.end > cls.name.span.start);
}

#[test]
fn test_identifier_with_underscore() {
    let src = "void _privateFunc() {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert_eq!(fun.name.name, "_privateFunc");
}

// ── String literal tests ──────────────────────────────────────────────────────

#[test]
fn test_string_lit_node_value() {
    let src = r#"var s = "hello";"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    let Some(init) = &v.declarators[0].initializer else { panic!("expected init") };
    let Expr::StringLit(lit) = init else { panic!("expected string lit") };
    // The value should be decoded (without quotes)
    assert!(!lit.value.is_empty());
    // The raw should preserve original text
    assert!(!lit.raw.is_empty());
}

#[test]
fn test_string_lit_node_span() {
    let src = r#"var s = "hello";"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    let Some(init) = &v.declarators[0].initializer else { panic!("expected init") };
    let Expr::StringLit(lit) = init else { panic!("expected string lit") };
    assert!(lit.span.end > lit.span.start);
}

// ── Class member tests ────────────────────────────────────────────────────────

#[test]
fn test_class_member_span() {
    let src = "class Foo { int x; void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.members.len(), 2);

    // Each member should have a span
    for member in &cls.members {
        let member_span = member.span();
        assert!(member_span.end > member_span.start);
    }
}

#[test]
fn test_field_declarator_span() {
    let src = "class Foo { int x, y, z; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Field(fld) = &cls.members[0] else { panic!("expected field") };
    assert_eq!(fld.declarators.len(), 3);

    for declarator in &fld.declarators {
        assert!(declarator.span.end > declarator.span.start);
    }
}

// ── Function type tests ───────────────────────────────────────────────────────

#[test]
fn test_function_type_span() {
    let src = "typedef Callback = void Function(int);";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::TypeAlias(alias) = &prog.declarations[0] else { panic!("expected type alias") };
    let span = alias.aliased.span();
    assert!(span.end > span.start);
}

// ── Named type tests ─────────────────────────────────────────────────────────

#[test]
fn test_named_type_simple() {
    let src = "int x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    let Some(typ) = &v.var_type else { panic!("expected type") };
    let DartType::Named(named) = typ else { panic!("expected named type") };
    assert_eq!(named.segments.len(), 1);
}

#[test]
fn test_named_type_qualified() {
    let src = "dart.core.List x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    let Some(typ) = &v.var_type else { panic!("expected type") };
    let DartType::Named(named) = typ else { panic!("expected named type") };
    assert_eq!(named.segments.len(), 3);
}

#[test]
fn test_named_type_with_args() {
    let src = "List<String> list;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    let Some(typ) = &v.var_type else { panic!("expected type") };
    let DartType::Named(named) = typ else { panic!("expected named type") };
    assert_eq!(named.type_args.len(), 1);
}

#[test]
fn test_named_type_nullable() {
    let src = "int? x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    let Some(typ) = &v.var_type else { panic!("expected type") };
    assert!(typ.is_nullable());
}

// ── Formal param tests ────────────────────────────────────────────────────────

#[test]
fn test_formal_param_positional() {
    let src = "void foo(int x) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert_eq!(fun.params.positional.len(), 1);
}

#[test]
fn test_formal_param_named() {
    let src = "void foo({int x}) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert_eq!(fun.params.named.len(), 1);
}

#[test]
fn test_formal_param_optional() {
    let src = "void foo([int x]) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert_eq!(fun.params.optional_positional.len(), 1);
}

#[test]
fn test_formal_param_multiple_kinds() {
    let src = "void foo(int a, [int b], {int c}) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert_eq!(fun.params.positional.len(), 1);
    assert_eq!(fun.params.optional_positional.len(), 1);
    assert_eq!(fun.params.named.len(), 1);
}

// ── Local declarations tests ──────────────────────────────────────────────────

#[test]
fn test_local_var_decl() {
    let src = "main() { int x = 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::LocalVar(local_var) = &b.stmts[0] else { panic!("expected local var") };
            assert_eq!(local_var.declarators.len(), 1);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_local_var_final() {
    let src = "main() { final int x = 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::LocalVar(local_var) = &b.stmts[0] else { panic!("expected local var") };
            assert!(local_var.is_final);
        }
        _ => panic!("expected block body"),
    }
}

// ── Type param tests ──────────────────────────────────────────────────────────

#[test]
fn test_type_param_simple() {
    let src = "class Box<T> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.type_params.len(), 1);
    assert_eq!(cls.type_params[0].name.name, "T");
}

#[test]
fn test_type_param_with_bound() {
    let src = "class Box<T extends num> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.type_params.len(), 1);
    assert!(cls.type_params[0].bound.is_some());
}

#[test]
fn test_type_params_multiple() {
    let src = "class Map<K, V> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.type_params.len(), 2);
}

// ── Annotation tests ──────────────────────────────────────────────────────────

#[test]
fn test_annotation_simple() {
    let src = "@deprecated class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.annotations.len(), 1);
}

#[test]
fn test_annotation_with_args() {
    let src = r#"@Deprecated('use Bar') class Foo {}"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.annotations.len(), 1);
    assert!(cls.annotations[0].args.is_some());
}

// ── Constructor tests ─────────────────────────────────────────────────────────

#[test]
fn test_constructor_default() {
    let src = "class Foo { Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Constructor(ctor) = &cls.members[0] else { panic!("expected constructor") };
    assert_eq!(ctor.name.name, "Foo");
    assert!(!ctor.is_const);
    assert!(!ctor.is_factory);
}

#[test]
fn test_constructor_const() {
    let src = "class Foo { const Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Constructor(ctor) = &cls.members[0] else { panic!("expected constructor") };
    assert!(ctor.is_const);
}

#[test]
fn test_constructor_factory() {
    let src = "class Foo { factory Foo() => Foo._(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Constructor(ctor) = &cls.members[0] else { panic!("expected constructor") };
    assert!(ctor.is_factory);
}

#[test]
fn test_constructor_named() {
    let src = "class Foo { Foo.named(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Constructor(ctor) = &cls.members[0] else { panic!("expected constructor") };
    assert!(ctor.constructor_name.is_some());
}

// ── Method tests ──────────────────────────────────────────────────────────────

#[test]
fn test_method_basic() {
    let src = "class Foo { void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Method(meth) = &cls.members[0] else { panic!("expected method") };
    assert_eq!(meth.name.name, "bar");
}

#[test]
fn test_method_async() {
    let src = "class Foo { async void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Method(meth) = &cls.members[0] else { panic!("expected method") };
    assert!(meth.is_async);
}

#[test]
fn test_method_static() {
    let src = "class Foo { static void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Method(meth) = &cls.members[0] else { panic!("expected method") };
    assert!(meth.is_static);
}

#[test]
fn test_method_abstract() {
    let src = "abstract class Foo { void bar(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Method(meth) = &cls.members[0] else { panic!("expected method") };
    assert!(meth.is_abstract);
}

// ── Getter/Setter tests ───────────────────────────────────────────────────────

#[test]
fn test_getter() {
    let src = "class Foo { int get value => 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(matches!(&cls.members[0], ClassMember::Getter(_)));
}

#[test]
fn test_setter() {
    let src = "class Foo { void set value(int x) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(matches!(&cls.members[0], ClassMember::Setter(_)));
}

// ── Operator tests ────────────────────────────────────────────────────────────

#[test]
fn test_operator_overload() {
    let src = "class Foo { Foo operator+(Foo other) => this; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(matches!(&cls.members[0], ClassMember::Operator(_)));
}

// ── Error node tests ──────────────────────────────────────────────────────────

#[test]
fn test_error_node_graceful() {
    let src = "this is invalid {";
    let (prog, _errors) = parse(src);
    // Parser should not panic, should produce at least an error node or partial tree
    let _ = prog;
}
