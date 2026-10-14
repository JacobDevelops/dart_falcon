use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

#[test]
fn test_identifier_basic() {
    let src = "class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.name.name, "Foo");
    assert!(cls.name.span.end > cls.name.span.start);
}

#[test]
fn test_identifier_with_underscore() {
    let src = "void _privateFunc() {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert_eq!(fun.name.name, "_privateFunc");
}

#[test]
fn test_string_lit_node_value() {
    let src = r#"var s = "hello";"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    let Some(init) = &v.declarators[0].initializer else {
        panic!("expected init")
    };
    let Expr::StringLit(lit) = init else {
        panic!("expected string lit")
    };
    assert_eq!(lit.value, "hello");
    assert_eq!(lit.raw, "\"hello\"");
}

#[test]
fn test_class_member_span() {
    let src = "class Foo { int x; void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
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

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Field(fld) = &cls.members[0] else {
        panic!("expected field")
    };
    assert_eq!(fld.declarators.len(), 3);

    for declarator in &fld.declarators {
        assert!(declarator.span.end > declarator.span.start);
    }
}

#[test]
fn test_named_type_simple() {
    let src = "int x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    let Some(typ) = &v.var_type else {
        panic!("expected type")
    };
    let DartType::Named(named) = typ else {
        panic!("expected named type")
    };
    assert_eq!(named.segments.len(), 1);
}

#[test]
fn test_named_type_qualified() {
    let src = "dart.core.List x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    let Some(typ) = &v.var_type else {
        panic!("expected type")
    };
    let DartType::Named(named) = typ else {
        panic!("expected named type")
    };
    assert_eq!(named.segments.len(), 3);
}

#[test]
fn test_named_type_with_args() {
    let src = "List<String> list;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    let Some(typ) = &v.var_type else {
        panic!("expected type")
    };
    let DartType::Named(named) = typ else {
        panic!("expected named type")
    };
    assert_eq!(named.type_args.len(), 1);
}

#[test]
fn test_named_type_nullable() {
    let src = "int? x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    let Some(typ) = &v.var_type else {
        panic!("expected type")
    };
    assert!(typ.is_nullable());
}

#[test]
fn test_formal_param_positional() {
    let src = "void foo(int x) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let params = &fun.params().unwrap().params;
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].kind, ParamKind::Positional);
}

#[test]
fn test_formal_param_named() {
    let src = "void foo({int x}) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let params = &fun.params().unwrap().params;
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].kind, ParamKind::Named { required: false });
}

#[test]
fn test_formal_param_optional() {
    let src = "void foo([int x]) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let params = &fun.params().unwrap().params;
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].kind, ParamKind::OptionalPositional);
}

#[test]
fn test_formal_param_multiple_kinds() {
    let src = "void foo(int a, [int b], {int c}) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let params = &fun.params().unwrap().params;
    assert_eq!(params.len(), 3);
    assert_eq!(params[0].kind, ParamKind::Positional);
    assert_eq!(params[1].kind, ParamKind::OptionalPositional);
    assert_eq!(params[2].kind, ParamKind::Named { required: false });
    assert_eq!(
        params
            .iter()
            .map(|param| param.name.name.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[test]
fn test_formal_param_named_required_and_source_order() {
    let (prog, errors) = parse("void foo(int z, {required int y, int a = 1}) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let params = &fun.params().unwrap().params;
    assert_eq!(params.len(), 3);
    assert_eq!(
        params
            .iter()
            .map(|param| param.name.name.as_str())
            .collect::<Vec<_>>(),
        ["z", "y", "a"]
    );
    assert_eq!(params[0].kind, ParamKind::Positional);
    assert_eq!(params[1].kind, ParamKind::Named { required: true });
    assert_eq!(params[2].kind, ParamKind::Named { required: false });
    assert!(params[1].default_value.is_none());
    assert!(matches!(&params[2].default_value, Some(Expr::IntLit { value, .. }) if value == "1"));
}

#[test]
fn test_local_var_decl() {
    let src = "main() { int x = 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::LocalVar(local_var) = &b.stmts[0] else {
                panic!("expected local var")
            };
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

    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::LocalVar(local_var) = &b.stmts[0] else {
                panic!("expected local var")
            };
            assert_eq!(local_var.keyword, VarKeyword::Final);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_type_param_simple() {
    let src = "class Box<T> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params.len(), 1);
    assert_eq!(cls.type_params[0].name.name, "T");
}

#[test]
fn test_type_param_with_bound() {
    let src = "class Box<T extends num> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params.len(), 1);
    assert!(cls.type_params[0].bound.is_some());
}

#[test]
fn test_type_params_multiple() {
    let src = "class Map<K, V> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params.len(), 2);
}

#[test]
fn test_annotation_simple() {
    let src = "@deprecated class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.annotations.len(), 1);
}

#[test]
fn test_annotation_with_args() {
    let src = r#"@Deprecated('use Bar') class Foo {}"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.annotations.len(), 1);
    assert!(cls.annotations[0].args.is_some());
}

#[test]
fn test_constructor_default() {
    let src = "class Foo { Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = &cls.members[0] else {
        panic!("expected constructor")
    };
    assert_eq!(ctor.name.name, "Foo");
    assert!(!ctor.is_const);
    assert!(!ctor.is_factory);
}

#[test]
fn test_constructor_const() {
    let src = "class Foo { const Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = &cls.members[0] else {
        panic!("expected constructor")
    };
    assert!(ctor.is_const);
}

#[test]
fn test_constructor_factory() {
    let src = "class Foo { factory Foo() => Foo._(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = &cls.members[0] else {
        panic!("expected constructor")
    };
    assert!(ctor.is_factory);
}

#[test]
fn test_constructor_named() {
    let src = "class Foo { Foo.named(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = &cls.members[0] else {
        panic!("expected constructor")
    };
    assert!(ctor.constructor_name.is_some());
}

#[test]
fn test_method_basic() {
    let src = "class Foo { void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = &cls.members[0] else {
        panic!("expected method")
    };
    assert_eq!(meth.name.name, "bar");
}

#[test]
fn test_method_async() {
    let src = "class Foo { async void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = &cls.members[0] else {
        panic!("expected method")
    };
    assert_eq!(meth.async_marker, AsyncMarker::Async);
}

#[test]
fn test_method_static() {
    let src = "class Foo { static void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = &cls.members[0] else {
        panic!("expected method")
    };
    assert!(meth.is_static);
}

#[test]
fn test_method_abstract() {
    let src = "abstract class Foo { void bar(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = &cls.members[0] else {
        panic!("expected method")
    };
    assert!(meth.is_abstract);
}

#[test]
fn nullable_written_void_recovers_as_canonical_void_without_diagnostics() {
    let (program, errors) = parse("void? f() {}");
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = &program.declarations[0] else {
        panic!("expected function");
    };
    assert!(matches!(function.return_type, Some(DartType::Void { .. })));
}

#[test]
fn nullable_dynamic_has_one_canonical_representation() {
    let (program, errors) = parse("dynamic? x; Map<String, dynamic?> y;");
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(first) = &program.declarations[0] else {
        panic!("expected variable");
    };
    assert!(matches!(first.var_type, Some(DartType::Dynamic { .. })));
    let TopLevelDecl::Variable(second) = &program.declarations[1] else {
        panic!("expected variable");
    };
    let Some(DartType::Named(map)) = &second.var_type else {
        panic!("expected map type");
    };
    assert!(matches!(map.type_args[1], DartType::Dynamic { .. }));
}

#[test]
fn string_delimiters_preserve_content_and_escape_text() {
    for (raw, content, quote, triple, is_raw, offset) in [
        ("'hello'", "hello", b'\'', false, false, 1),
        ("\"hello\"", "hello", b'"', false, false, 1),
        ("'''hello'''", "hello", b'\'', true, false, 3),
        ("\"\"\"hello\"\"\"", "hello", b'"', true, false, 3),
        ("r'\\n'", "\\n", b'\'', false, true, 2),
        ("r\"\"\"\\n\"\"\"", "\\n", b'"', true, true, 4),
        ("'\\n'", "\\n", b'\'', false, false, 1),
    ] {
        let delimiter = StringLitNode::delimiter(raw).unwrap();
        assert_eq!(delimiter.content, content);
        assert_eq!(delimiter.quote, quote);
        assert_eq!(delimiter.triple, triple);
        assert_eq!(delimiter.is_raw, is_raw);
        assert_eq!(delimiter.content_offset, offset);
        assert!(delimiter.terminated);
        let (program, errors) = parse(&format!("var s = {raw};"));
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Variable(variable) = &program.declarations[0] else {
            panic!("expected variable");
        };
        let Some(Expr::StringLit(literal)) = &variable.declarators[0].initializer else {
            panic!("expected string");
        };
        assert_eq!(literal.value, content);
        assert_eq!(literal.raw, raw);
    }
}

#[test]
fn unterminated_string_delimiters_keep_recovery_content() {
    for raw in ["'hello", "\"hello", "'''hello", "r'''hello"] {
        let delimiter = StringLitNode::delimiter(raw).unwrap();
        assert!(!delimiter.terminated);
        assert_eq!(delimiter.content, "hello");
    }
    assert!(StringLitNode::delimiter("identifier").is_none());
}

#[test]
fn adjacent_string_values_concatenate_without_decoding_escapes() {
    let (program, errors) = parse("var s = 'a\\n' r\"b\\t\";");
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(variable) = &program.declarations[0] else {
        panic!("expected variable");
    };
    let Some(Expr::StringLit(literal)) = &variable.declarators[0].initializer else {
        panic!("expected string");
    };
    assert_eq!(literal.value, "a\\nb\\t");
    assert_eq!(literal.raw, "'a\\n'r\"b\\t\"");
}
