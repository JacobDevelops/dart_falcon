use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;

#[test]
fn test_identifier_basic() {
    let src = "class Foo {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.name().name(), "Foo");
    assert!(cls.name().span().end > cls.name().span().start);
}

#[test]
fn test_identifier_with_underscore() {
    let src = "void _privateFunc() {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert_eq!(fun.name().name(), "_privateFunc");
}

#[test]
fn test_string_lit_node_value() {
    let src = r#"var s = "hello";"#;
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    let Some(init) = v.declarators().next().unwrap().initializer() else {
        panic!("expected init")
    };
    let Expr::StringLit(lit) = init else {
        panic!("expected string lit")
    };
    assert_eq!(lit.value(), "hello");
    assert_eq!(lit.raw(), "\"hello\"");
}

#[test]
fn test_class_member_span() {
    let src = "class Foo { int x; void bar() {} }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.members().len(), 2);

    // Each member should have a span
    for member in cls.members() {
        let member_span = member.span();
        assert!(member_span.end > member_span.start);
    }
}

#[test]
fn test_field_declarator_span() {
    let src = "class Foo { int x, y, z; }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Field(fld) = cls.members().next().unwrap() else {
        panic!("expected field")
    };
    assert_eq!(fld.declarators().len(), 3);

    for declarator in fld.declarators() {
        assert!(declarator.span().end > declarator.span().start);
    }
}

#[test]
fn test_named_type_simple() {
    let src = "int x;";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    let Some(typ) = v.var_type() else {
        panic!("expected type")
    };
    let DartType::Named(named) = typ else {
        panic!("expected named type")
    };
    assert_eq!(named.segments().count(), 1);
}

#[test]
fn test_named_type_qualified() {
    let src = "dart.core.List x;";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    let Some(typ) = v.var_type() else {
        panic!("expected type")
    };
    let DartType::Named(named) = typ else {
        panic!("expected named type")
    };
    assert_eq!(named.segments().count(), 3);
}

#[test]
fn test_named_type_with_args() {
    let src = "List<String> list;";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    let Some(typ) = v.var_type() else {
        panic!("expected type")
    };
    let DartType::Named(named) = typ else {
        panic!("expected named type")
    };
    assert_eq!(named.type_args().len(), 1);
}

#[test]
fn test_named_type_nullable() {
    let src = "int? x;";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    let Some(typ) = v.var_type() else {
        panic!("expected type")
    };
    assert!(typ.is_nullable());
}

#[test]
fn test_formal_param_positional() {
    let src = "void foo(int x) {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    let params = fun.params().unwrap().params();
    assert_eq!(params.len(), 1);
    assert_eq!(params.clone().next().unwrap().kind(), ParamKind::Positional);
}

#[test]
fn test_formal_param_named() {
    let src = "void foo({int x}) {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    let params = fun.params().unwrap().params();
    assert_eq!(params.len(), 1);
    assert_eq!(
        params.clone().next().unwrap().kind(),
        ParamKind::Named { required: false }
    );
}

#[test]
fn test_formal_param_optional() {
    let src = "void foo([int x]) {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    let params = fun.params().unwrap().params();
    assert_eq!(params.len(), 1);
    assert_eq!(
        params.clone().next().unwrap().kind(),
        ParamKind::OptionalPositional
    );
}

#[test]
fn test_formal_param_multiple_kinds() {
    let src = "void foo(int a, {int c}) {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    let params = fun.params().unwrap().params();
    assert_eq!(params.len(), 2);
    assert_eq!(params.clone().next().unwrap().kind(), ParamKind::Positional);
    assert_eq!(
        params.clone().nth(1).unwrap().kind(),
        ParamKind::Named { required: false }
    );
    // Optional-positional and named groups cannot both appear.
    assert!(
        !parse("void foo(int a, [int b], {int c}) {}")
            .errors
            .is_empty()
    );
}

#[test]
fn test_formal_param_named_required_and_source_order() {
    let source = "void foo(int z, {required int y, int a = 1}) {}";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    let params = fun.params().unwrap().params();
    assert_eq!(params.len(), 3);
    assert_eq!(
        params
            .clone()
            .map(|param| param.name().name())
            .collect::<Vec<_>>(),
        ["z", "y", "a"]
    );
    assert_eq!(params.clone().next().unwrap().kind(), ParamKind::Positional);
    assert_eq!(
        params.clone().nth(1).unwrap().kind(),
        ParamKind::Named { required: true }
    );
    assert_eq!(
        params.clone().nth(2).unwrap().kind(),
        ParamKind::Named { required: false }
    );
    assert!(params.clone().nth(1).unwrap().default_value().is_none());
    assert!(
        matches!(&params.clone().nth(2).unwrap().default_value(), Some(Expr::IntLit(value)) if value.value() == "1")
    );
}

#[test]
fn test_local_var_decl() {
    let src = "main() { int x = 42; }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::LocalVar(local_var) = b.stmts().next().unwrap() else {
                panic!("expected local var")
            };
            assert_eq!(local_var.declarators().len(), 1);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_local_var_final() {
    let src = "main() { final int x = 42; }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::LocalVar(local_var) = b.stmts().next().unwrap() else {
                panic!("expected local var")
            };
            assert_eq!(local_var.keyword(), VarKeyword::Final);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_type_param_simple() {
    let src = "class Box<T> {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params().len(), 1);
    assert_eq!(cls.type_params().next().unwrap().name().name(), "T");
}

#[test]
fn test_type_param_with_bound() {
    let src = "class Box<T extends num> {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params().len(), 1);
    assert!(cls.type_params().next().unwrap().bound().is_some());
}

#[test]
fn test_type_params_multiple() {
    let src = "class Map<K, V> {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params().len(), 2);
}

#[test]
fn test_annotation_simple() {
    let src = "@deprecated class Foo {}";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.annotations().len(), 1);
}

#[test]
fn test_annotation_with_args() {
    let src = r#"@Deprecated('use Bar') class Foo {}"#;
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.annotations().len(), 1);
    assert!(cls.annotations().next().unwrap().args().is_some());
}

#[test]
fn test_constructor_default() {
    let src = "class Foo { Foo(); }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = cls.members().next().unwrap() else {
        panic!("expected constructor")
    };
    assert_eq!(ctor.name().name(), "Foo");
    assert!(!ctor.is_const());
    assert!(!ctor.is_factory());
}

#[test]
fn test_constructor_const() {
    let src = "class Foo { const Foo(); }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = cls.members().next().unwrap() else {
        panic!("expected constructor")
    };
    assert!(ctor.is_const());
}

#[test]
fn test_constructor_factory() {
    let src = "class Foo { factory Foo() => Foo._(); }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = cls.members().next().unwrap() else {
        panic!("expected constructor")
    };
    assert!(ctor.is_factory());
}

#[test]
fn test_constructor_named() {
    let src = "class Foo { Foo.named(); }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = cls.members().next().unwrap() else {
        panic!("expected constructor")
    };
    assert!(ctor.constructor_name().is_some());
}

#[test]
fn test_method_basic() {
    let src = "class Foo { void bar() {} }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = cls.members().next().unwrap() else {
        panic!("expected method")
    };
    assert_eq!(meth.name().name(), "bar");
}

#[test]
fn test_method_async() {
    let src = "class Foo { async void bar() {} }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = cls.members().next().unwrap() else {
        panic!("expected method")
    };
    assert_eq!(meth.async_marker(), AsyncMarker::Async);
}

#[test]
fn test_method_static() {
    let src = "class Foo { static void bar() {} }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = cls.members().next().unwrap() else {
        panic!("expected method")
    };
    assert!(meth.is_static());
}

#[test]
fn test_method_abstract() {
    let src = "abstract class Foo { void bar(); }";
    let source = src;
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let prog = tree.program(source);
    assert!(errors.is_empty());

    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = cls.members().next().unwrap() else {
        panic!("expected method")
    };
    assert!(meth.is_abstract());
}

#[test]
fn nullable_written_void_recovers_as_canonical_void_without_diagnostics() {
    let source = "void? f() {}";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!("expected function");
    };
    assert!(matches!(function.return_type(), Some(DartType::Void(_))));
}

#[test]
fn nullable_dynamic_has_one_canonical_representation() {
    let source = "dynamic? x; Map<String, dynamic?> y;";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(first) = program.declarations().next().unwrap() else {
        panic!("expected variable");
    };
    assert!(matches!(first.var_type(), Some(DartType::Dynamic(_))));
    let TopLevelDecl::Variable(second) = program.declarations().nth(1).unwrap() else {
        panic!("expected variable");
    };
    let Some(DartType::Named(map)) = second.var_type() else {
        panic!("expected map type");
    };
    assert!(matches!(
        map.type_args().nth(1).unwrap(),
        DartType::Dynamic(_)
    ));
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
        let source = &format!("var s = {raw};");
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
            panic!("expected variable");
        };
        let Some(Expr::StringLit(literal)) = variable.declarators().next().unwrap().initializer()
        else {
            panic!("expected string");
        };
        let delimiter = literal.delimiter().unwrap();
        assert_eq!(delimiter.content, content);
        assert_eq!(delimiter.quote, quote);
        assert_eq!(delimiter.triple, triple);
        assert_eq!(delimiter.is_raw, is_raw);
        assert_eq!(delimiter.content_offset, offset);
        assert!(delimiter.terminated);
        assert_eq!(literal.value(), content);
        assert_eq!(literal.raw(), raw);
    }
}

#[test]
fn unterminated_string_delimiters_keep_recovery_content() {
    for raw in ["'hello", "\"hello", "'''hello", "r'''hello"] {
        let delimiter = parse_string_delimiter(raw).unwrap();
        assert!(!delimiter.terminated);
        assert_eq!(delimiter.content, "hello");
    }
    assert!(parse_string_delimiter("identifier").is_none());
}

#[test]
fn adjacent_string_values_concatenate_without_decoding_escapes() {
    let source = "var s = 'a\\n' r\"b\\t\";";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
        panic!("expected variable");
    };
    let Some(Expr::StringLit(literal)) = variable.declarators().next().unwrap().initializer()
    else {
        panic!("expected string");
    };
    assert_eq!(literal.value(), "a\\nb\\t");
    assert_eq!(literal.raw(), "'a\\n'r\"b\\t\"");
    assert!(literal.delimiter().is_none());
}

#[test]
fn operator_markers_and_constructor_context_are_preserved() {
    for (marker, expected) in [
        ("", AsyncMarker::Sync),
        ("async", AsyncMarker::Async),
        ("async*", AsyncMarker::AsyncStar),
        ("sync*", AsyncMarker::SyncStar),
    ] {
        let source = &format!("class C {{ C operator +(o) {marker} {{}} }}");
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Class(class) = program.declarations().next().unwrap() else {
            panic!("class");
        };
        let ClassMember::Operator(operator) = class.members().next().unwrap() else {
            panic!("operator");
        };
        assert_eq!(operator.async_marker(), expected);
    }
    let source = "class C { C() : m = Mode.async { var await = 1; } }";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Class(class) = program.declarations().next().unwrap() else {
        panic!("class");
    };
    let ClassMember::Constructor(constructor) = class.members().next().unwrap() else {
        panic!("constructor");
    };
    let Some(FunctionBody::Block(body)) = constructor.body() else {
        panic!("body");
    };
    let Stmt::LocalVar(variable) = body.stmts().next().unwrap() else {
        panic!("variable");
    };
    assert_eq!(
        variable.declarators().next().unwrap().name().name(),
        "await"
    );
}

#[test]
fn annotated_local_declaration_keeps_all_declarators() {
    let source = "void f() { @a int x = 1, y = 2; }";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!("function");
    };
    let Some(FunctionBody::Block(body)) = function.body() else {
        panic!("body");
    };
    let Stmt::LocalVar(variable) = body.stmts().next().unwrap() else {
        panic!("variable");
    };
    assert_eq!(variable.declarators().len(), 2);
    for (declarator, (name, value)) in variable.declarators().zip([("x", "1"), ("y", "2")]) {
        assert_eq!(declarator.name().name(), name);
        assert!(
            matches!(&declarator.initializer(), Some(Expr::IntLit(actual)) if actual.value() == value)
        );
    }
}

#[test]
fn adjacent_delimiters_never_mix_raw_and_interpolated_segments() {
    for (raw, has_raw_segment) in [
        ("'a' r'\\d'", true),
        ("r'\\d' 'a'", true),
        ("'a''b'", false),
        ("'''a'''r'''\\d'''", true),
        ("'${\"x\"}' r'\\d'", true),
    ] {
        let source = format!("var s = {raw};");
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{raw}: {errors:?}");
        let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
            panic!("variable");
        };
        let Some(Expr::StringLit(literal)) = variable.declarators().next().unwrap().initializer()
        else {
            panic!("literal");
        };
        assert!(literal.is_adjacent());
        assert_eq!(literal.has_raw_segment(), has_raw_segment, "{raw}");
        assert!(literal.delimiter().is_none(), "{raw}");
        let segments: Vec<_> = literal.segments().collect();
        assert_eq!(segments.len(), 2, "{raw}");
        assert_eq!(segments.iter().any(|(lit, _)| lit.is_raw), has_raw_segment);
        for (delimiter, span) in segments {
            assert!(delimiter.terminated);
            assert_eq!(
                &source[span.start + delimiter.content_offset
                    ..span.end - if delimiter.triple { 3 } else { 1 }],
                delimiter.content
            );
        }
    }
}

#[test]
fn statement_loops_share_complete_headers() {
    for (clause, kind, updates) in [
        ("var i = 0; i < 3; i++, i += 1", "declaration", 2),
        ("i = 0; i < 3; i++", "expression", 1),
        ("final x in xs", "for-in", 0),
        ("final (x, y) in xs", "pattern", 0),
    ] {
        let source = &format!("void f() {{ for ({clause}) {{}} }}");
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{clause}: {errors:?}");
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("function");
        };
        let Some(FunctionBody::Block(body)) = function.body() else {
            panic!("body");
        };
        let Stmt::For(statement) = body.stmts().next().unwrap() else {
            panic!("for");
        };
        let actual = match &statement.header().init() {
            Some(ForInit::VarDecl(_)) => "declaration",
            Some(ForInit::Exprs(_)) => "expression",
            Some(ForInit::ForIn(_)) => "for-in",
            Some(ForInit::PatternForIn(_)) => "pattern",
            None => "empty",
        };
        assert_eq!(actual, kind);
        assert_eq!(statement.header().updates().len(), updates);
        assert_eq!(statement.header().condition().is_some(), updates != 0);
    }
}

#[test]
fn grouping_parentheses_and_operator_spans_are_preserved() {
    let source = "var value = (( /* why */ 1)) == 2;";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
        panic!("expected variable");
    };
    let Expr::Binary(binary) = variable
        .declarators()
        .next()
        .unwrap()
        .initializer()
        .unwrap()
    else {
        panic!("expected binary");
    };
    let op_span = binary.op_span();
    let left = binary.left();
    assert_eq!(&source[op_span.start..op_span.end], "==");
    let Expr::Paren(paren) = left else {
        panic!("expected outer parentheses");
    };
    let span = paren.span();
    assert_eq!(&source[span.start..span.end], "(( /* why */ 1))");
    assert!(matches!(paren.expr(), Expr::Paren(_)));
    assert!(matches!(left.unparenthesized(), Expr::IntLit(_)));
}

#[test]
fn cascade_index_assignments_have_distinct_syntax() {
    let source = "var value = target..field /* [ */ = 1..[index] = 2;";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
        panic!("expected variable");
    };
    let Expr::Cascade(cascade) = variable
        .declarators()
        .next()
        .unwrap()
        .initializer()
        .unwrap()
    else {
        panic!("expected cascade");
    };
    assert!(matches!(
        cascade.sections().next().unwrap().ops().next().unwrap(),
        CascadeOp::Assign(_)
    ));
    assert!(matches!(
        cascade.sections().nth(1).unwrap().ops().next().unwrap(),
        CascadeOp::IndexAssign(_)
    ));
}

#[test]
fn shared_visitor_observes_each_effective_constructor_once() {
    use falcon_syntax::cst::visit::Visitor;

    #[derive(Default)]
    struct Constructors {
        primary: usize,
        ordinary: usize,
        bodies: usize,
        defaults: usize,
    }
    impl<'a> Visitor<'a> for Constructors {
        fn visit_constructor(&mut self, constructor: ConstructorRef<'a>) {
            if constructor.is_primary {
                self.primary += 1;
            } else {
                self.ordinary += 1;
            }
            self.bodies += usize::from(constructor.body.is_some());
        }

        fn visit_expr(&mut self, expression: Expr<'a>) {
            if matches!(expression, Expr::IntLit(value) if value.value() == "42") {
                self.defaults += 1;
            }
            falcon_syntax::cst::visit::walk_expr(self, expression);
        }
    }

    let source = "class C([int x = 42]) { this {} C.other() : this(); } \
        mixin class M() {} \
        enum E([int x = 42]) { first; } \
        extension type X(int value) { X.other(this.value); }";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut visitor = Constructors::default();
    visitor.visit_program(program);
    assert_eq!(visitor.primary, 4);
    assert_eq!(visitor.ordinary, 2);
    assert_eq!(visitor.bodies, 1);
    assert_eq!(visitor.defaults, 2);
}

#[test]
fn expression_spans_end_at_consumed_tokens_not_following_trivia() {
    for expression in [
        "(target)",
        "target.field",
        "target[0]",
        "target.method(1)",
        "target + 1",
        "target..method()()",
        "a < b >> c",
    ] {
        let source = format!("var value = {expression} /* trailing */ \n;");
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
            panic!("expected variable");
        };
        let initializer = variable
            .declarators()
            .next()
            .unwrap()
            .initializer()
            .unwrap();
        let start = "var value = ".len();
        assert_eq!(
            (initializer.span().start, initializer.span().end),
            (start, start + expression.len()),
            "{source}",
        );
    }
}

#[test]
fn nested_type_spans_end_after_each_consumed_shift_token_delimiter() {
    for names in [
        &["List<List<int>>", "List<int>", "int"][..],
        &[
            "List<List<List<int>>>",
            "List<List<int>>",
            "List<int>",
            "int",
        ][..],
    ] {
        let source = format!("{} /* trailing */ value;", names[0]);
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
            panic!("expected variable");
        };
        let mut ty = variable.var_type().unwrap();
        for expected in names {
            let DartType::Named(named) = ty else {
                panic!("expected named type");
            };
            assert_eq!(&source[named.span().start..named.span().end], *expected);
            if let Some(argument) = named.type_args().first() {
                ty = argument;
            }
        }
    }
}

#[test]
fn cascade_chained_invocations_preserve_type_arguments_and_delimiters() {
    let source = "var value = target..method()()<int>(1)..[0]() /* trailing */ ;";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
        panic!("expected variable");
    };
    let Expr::Cascade(cascade) = variable
        .declarators()
        .next()
        .unwrap()
        .initializer()
        .unwrap()
    else {
        panic!("expected cascade");
    };
    assert_eq!(cascade.sections().len(), 2);
    let first = cascade.sections().next().unwrap();
    let second = cascade.sections().nth(1).unwrap();
    assert_eq!(first.ops().len(), 3);
    let CascadeOp::Call(call) = first.ops().next().unwrap() else {
        panic!("expected call");
    };
    assert_eq!(call.name().name(), "method");
    let CascadeOp::Invoke(first_invoke) = first.ops().nth(1).unwrap() else {
        panic!("expected first invocation");
    };
    assert!(first_invoke.type_args().is_empty());
    let CascadeOp::Invoke(second_invoke) = first.ops().nth(2).unwrap() else {
        panic!("expected second invocation");
    };
    assert_eq!(second_invoke.type_args().len(), 1);
    assert_eq!(second.ops().len(), 2);
    let CascadeOp::Index(index) = second.ops().next().unwrap() else {
        panic!("expected index");
    };
    let CascadeOp::Invoke(invoke) = second.ops().nth(1).unwrap() else {
        panic!("expected invocation");
    };
    assert!(invoke.type_args().is_empty());
    assert_eq!(
        &source[first.span().start..first.span().end],
        "..method()()<int>(1)"
    );
    assert_eq!(&source[second.span().start..second.span().end], "..[0]()");
    let args = second_invoke.args();
    assert_eq!(&source[args.span().start..args.span().end], "(1)");
    let ty = second_invoke.type_args().next().unwrap();
    assert_eq!(&source[ty.span().start..ty.span().end], "int");
    assert_eq!(&source[index.span().start..index.span().end], "[0]");
}

#[test]
fn cascade_null_assertions_preserve_selectors_and_following_statements() {
    let source = "void f() { c..clear()..parent!.go(c); c.go; }";
    let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
    let program = tree.program(source);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!("expected function");
    };
    let Some(FunctionBody::Block(block)) = function.body() else {
        panic!("expected block");
    };
    assert_eq!(block.stmts().len(), 2);
    let Stmt::Expr(statement) = block.stmts().next().unwrap() else {
        panic!("expected expression statement");
    };
    let Expr::Cascade(cascade) = statement.expr() else {
        panic!("expected cascade statement");
    };
    let span = cascade.span();
    assert_eq!(&source[span.start..span.end], "c..clear()..parent!.go(c)");
    assert_eq!(cascade.sections().len(), 2);
    let first = cascade.sections().next().unwrap();
    assert_eq!(first.ops().len(), 1);
    assert!(
        matches!(first.ops().next().unwrap(), CascadeOp::Call(call) if call.name().name() == "clear")
    );
    let second = cascade.sections().nth(1).unwrap();
    assert_eq!(second.ops().len(), 3);
    let CascadeOp::Field(field) = second.ops().next().unwrap() else {
        panic!("expected field");
    };
    let CascadeOp::NullAssert(assertion) = second.ops().nth(1).unwrap() else {
        panic!("expected assertion");
    };
    let CascadeOp::Call(call) = second.ops().nth(2).unwrap() else {
        panic!("expected call");
    };
    assert_eq!(field.name().name(), "parent");
    let span = assertion.span();
    assert_eq!(&source[span.start..span.end], "!");
    assert_eq!(call.name().name(), "go");
    assert_eq!(call.args().positional().len(), 1);
    let Stmt::Expr(statement) = block.stmts().nth(1).unwrap() else {
        panic!("expected tearoff statement");
    };
    assert!(matches!(statement.expr(), Expr::Field(field) if field.field().name() == "go"));
}

#[test]
fn legacy_typedef_function_span_ends_at_parameter_closure() {
    for declaration in ["typedef void F(int x);", "typedef F(int x);"] {
        let source = format!("{declaration} /* trailing trivia */\n\nvoid next() {{}}");
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::TypeAlias(alias) = program.declarations().next().unwrap() else {
            panic!("expected typedef");
        };
        let DartType::Function(function) = alias.aliased() else {
            panic!("expected synthesized function type");
        };
        let start = "typedef ".len();
        let end = declaration.len() - 1;
        assert_eq!(function.span(), Span::new(start, end));
        assert_eq!(alias.span(), Span::new(0, declaration.len()));
        assert_eq!(alias.legacy_params().unwrap().span().end, end);
    }
}

#[test]
fn local_variable_spans_preserve_attached_documentation_and_metadata() {
    for (documentation, first_documentation) in [
        ("/// first\n/// second\n", "/// first"),
        ("/// first\n// ordinary\n", "/// first"),
        ("/** first */\n/** second */\n", "/** second */"),
        ("/// first\n// ordinary\n/// second\n", "/// first"),
        ("", "@deprecated"),
    ] {
        let source = format!("void f() {{\n{documentation}@deprecated\nfinal value = 1;\n}}");
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = function.body() else {
            panic!("expected block body");
        };
        let Stmt::LocalVar(declaration) = body.stmts().next().unwrap() else {
            panic!("expected local variable");
        };
        assert_eq!(declaration.annotations().len(), 1);
        assert_eq!(
            declaration.full_range(),
            Span::new(
                source.find(first_documentation).unwrap(),
                source.find(";\n}").unwrap() + 1,
            )
        );
        assert_eq!(
            declaration.range(),
            Span::new(
                source.find("@deprecated").unwrap(),
                source.find(";\n}").unwrap() + 1,
            )
        );
    }
}

#[test]
fn local_variable_keyword_spans_are_consumed_tokens_not_documentation_words() {
    for (declaration, keyword) in [
        ("var value = 1;", Some("var")),
        ("final value = 1;", Some("final")),
        ("const value = 1;", Some("const")),
        ("late /* final */ final int value = 1;", Some("final")),
        ("late var value = 1;", Some("var")),
        ("late int value = 1;", None),
        ("int value = 1;", None),
    ] {
        let source = format!(
            "void f() {{\n/// Documentation mentioning var final const.\n@deprecated\n{declaration}\n}}"
        );
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = function.body() else {
            panic!("expected block body");
        };
        let Stmt::LocalVar(declaration) = body.stmts().next().unwrap() else {
            panic!("expected local variable");
        };
        let expected = keyword.map(|keyword| {
            let start = source.rfind(keyword).unwrap();
            Span::new(start, start + keyword.len())
        });
        assert_eq!(declaration.keyword_span(), expected);
        assert_eq!(
            declaration.range().start,
            source.find("@deprecated").unwrap()
        );
        assert_eq!(declaration.full_range().start, source.find("///").unwrap());
    }
}

#[test]
fn for_loop_local_variable_keyword_spans_distinguish_keywordless_types() {
    for (declaration, keyword) in [
        ("var value = 1", Some("var")),
        ("final value = 1", Some("final")),
        ("const value = 1", Some("const")),
        ("late /* final */ final int value = 1", Some("final")),
        ("late int value = 1", None),
        ("int value = 1", None),
    ] {
        let source = format!("void f() {{ for ({declaration}; false;) {{}} }}");
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = function.body() else {
            panic!("expected block body");
        };
        let Stmt::For(statement) = body.stmts().next().unwrap() else {
            panic!("expected for statement");
        };
        let Some(ForInit::VarDecl(declaration)) = &statement.header().init() else {
            panic!("expected local for initializer");
        };
        let expected = keyword.map(|keyword| {
            let start = source.rfind(keyword).unwrap();
            Span::new(start, start + keyword.len())
        });
        assert_eq!(declaration.keyword_span(), expected);
    }
}

#[test]
fn pattern_declaration_keyword_span_is_the_consumed_token() {
    for (declaration, keyword) in [
        ("@pragma('final') final (a, b) = (1, 2);", "final"),
        ("@pragma('var') var [c, d] = [1, 2];", "var"),
    ] {
        let source =
            format!("void f() {{\n/// Documentation mentioning var final.\n{declaration}\n}}");
        let source = &source;
        let falcon_dart_parser::ParseResult { tree, errors, .. } = parse(source);
        let program = tree.program(source);
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = function.body() else {
            panic!("expected block body");
        };
        let Stmt::PatternDecl(parsed) = body.stmts().next().unwrap() else {
            panic!("expected pattern declaration");
        };
        let start = source.find(declaration).unwrap()
            + declaration.find(&format!(") {keyword} ")).unwrap()
            + 2;
        assert_eq!(
            parsed.keyword_span(),
            Span::new(start, start + keyword.len())
        );
    }
}
