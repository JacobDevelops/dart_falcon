//! Regression tests for declaration-level parser gaps (batch 1): each construct
//! must parse with zero errors and produce a faithful AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

#[path = "common/only_class.rs"]
mod common_class;
use common_class::only_class;

#[test]
fn test_untyped_async_method() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { foo() async {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let c = only_class(&prog);
    match &c.members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "foo");
            assert_eq!(m.async_marker, AsyncMarker::Async);
            assert!(m.return_type.is_none());
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_async_star_method() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { foo() async* {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.async_marker, AsyncMarker::AsyncStar);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_sync_star_method() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { foo() sync* {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.async_marker, AsyncMarker::SyncStar);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_generic_async_method() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { foo<T>() async {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.type_params.len(), 1);
            assert_eq!(m.async_marker, AsyncMarker::Async);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_field_named_async_still_parses() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { var async = 1; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Field(f) => assert_eq!(f.declarators[0].name.name, "async"),
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn test_method_named_async_still_parses() {
    // A member named `async` parses cleanly (as an untyped no-arg member).
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { async() {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let name = match &only_class(&prog).members[0] {
        ClassMember::Method(m) => m.name.name.clone(),
        other => panic!("expected method, got {other:?}"),
    };
    assert_eq!(name, "async");
}

#[test]
fn test_getter_named_sync_still_parses() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { int get sync => 1; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Getter(g) => assert_eq!(g.name.name, "sync"),
        other => panic!("expected getter, got {other:?}"),
    }
}

#[test]
fn test_index_operator() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { int operator [](int i) => 0; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Operator(op) => assert_eq!(op.op, "[]"),
        other => panic!("expected operator, got {other:?}"),
    }
}

#[test]
fn test_index_assign_operator() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { void operator []=(int i, int v) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Operator(op) => assert_eq!(op.op, "[]="),
        other => panic!("expected operator, got {other:?}"),
    }
}

#[test]
fn test_extension_type_const_named_ctor() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("extension type const Foo._(int x) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::ExtensionType(e) => {
            assert!(e.primary_constructor.is_const);
            assert_eq!(e.name.name, "Foo");
            assert_eq!(
                e.primary_constructor
                    .constructor_name
                    .as_ref()
                    .map(|i| i.name.as_str()),
                Some("_")
            );
            assert_eq!(e.representation().map(|p| p.name.name.as_str()), Some("x"));
        }
        other => panic!("expected extension type, got {other:?}"),
    }
}

#[test]
fn test_redirecting_factory() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { factory C() = D; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(c) => {
            assert!(c.is_factory);
            let redirect = c.redirect.as_ref().expect("redirect");
            assert!(c.body.is_none());
            match &redirect.type_ {
                DartType::Named(n) => assert_eq!(n.segments[0].name, "D"),
                other => panic!("expected named type, got {other:?}"),
            }
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_redirecting_factory_generic() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { factory C() = D<int>; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(c) => {
            let redirect = c.redirect.as_ref().expect("redirect");
            match &redirect.type_ {
                DartType::Named(n) => {
                    assert_eq!(n.segments[0].name, "D");
                    assert_eq!(n.type_args.len(), 1);
                }
                other => panic!("expected named type, got {other:?}"),
            }
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_typed_super_parameter() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C extends B { C(int super.x); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(c) => {
            let p = &c.params.params[0];
            assert_eq!(p.kind, ParamKind::Positional);
            assert_eq!(p.initializing, Some(InitializingFormal::Super));
            assert_eq!(p.name.name, "x");
            assert!(p.param_type.is_some());
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_conditional_import() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("import 'a.dart' if (dart.library.io) 'b.dart';");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let import = &prog.imports[0];
    assert_eq!(import.configurable_uris.len(), 1);
    let cu = &import.configurable_uris[0];
    assert_eq!(
        cu.test.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        vec!["dart", "library", "io"]
    );
    assert_eq!(cu.uri.value, "b.dart");
    assert!(cu.value.is_none());
}

#[test]
fn test_conditional_export_with_equality() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("export 'a.dart' if (dart.library.io == 'true') 'b.dart';");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let cu = &prog.exports[0].configurable_uris[0];
    assert_eq!(cu.value.as_ref().map(|s| s.value.as_str()), Some("true"));
}

#[test]
fn test_annotation_with_type_args() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("@Native<int Function()>() external void f();");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(func) => {
            let ann = &func.annotations[0];
            assert_eq!(ann.name[0].name, "Native");
            assert_eq!(ann.type_args.len(), 1);
        }
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn test_typed_field_formal() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { int x; C(int this.x); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[1] {
        ClassMember::Constructor(c) => {
            let p = &c.params.params[0];
            assert_eq!(p.kind, ParamKind::Positional);
            assert_eq!(p.initializing, Some(InitializingFormal::This));
            assert_eq!(p.name.name, "x");
            assert!(p.param_type.is_some());
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_mixin_application_class() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class MA = S with M;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::ClassTypeAlias(a) => {
            assert_eq!(a.name.name, "MA");
            match &a.superclass {
                DartType::Named(n) => assert_eq!(n.segments[0].name, "S"),
                other => panic!("expected named superclass, got {other:?}"),
            }
            assert_eq!(a.with_clause.len(), 1);
            assert!(a.implements.is_empty());
        }
        other => panic!("expected class type alias, got {other:?}"),
    }
}

#[test]
fn test_mixin_application_class_abstract_implements() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("abstract class MA = S with M implements I;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::ClassTypeAlias(a) => {
            assert!(a.modifiers.is_abstract);
            assert_eq!(a.implements.len(), 1);
        }
        other => panic!("expected class type alias, got {other:?}"),
    }
}

#[test]
fn test_legacy_typedef() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("typedef int Comparator(int a, int b);");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::TypeAlias(a) => {
            assert_eq!(a.name.name, "Comparator");
            match &a.aliased {
                DartType::Function(f) => {
                    assert!(f.return_type.is_some());
                    assert_eq!(f.params.len(), 2);
                }
                other => panic!("expected function type, got {other:?}"),
            }
        }
        other => panic!("expected type alias, got {other:?}"),
    }
}

#[test]
fn test_legacy_typedef_no_return_type() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("typedef Comparator(int a, int b);");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::TypeAlias(a) => {
            assert_eq!(a.name.name, "Comparator");
            match &a.aliased {
                DartType::Function(f) => assert!(f.return_type.is_none()),
                other => panic!("expected function type, got {other:?}"),
            }
        }
        other => panic!("expected type alias, got {other:?}"),
    }
}

#[test]
fn test_nullable_function_typed_formal() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("void f({int orElse()?}) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(func) => {
            let p = &func.params().unwrap().params[0];
            assert_eq!(p.kind, ParamKind::Named { required: false });
            assert_eq!(p.name.name, "orElse");
            assert!(p.function_params.is_some());
        }
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn test_abstract_mixin_class() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("abstract mixin class M {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::MixinClass(m) => assert!(m.is_abstract),
        other => panic!("expected mixin class, got {other:?}"),
    }
}

#[test]
fn test_non_abstract_mixin_class() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("mixin class M {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::MixinClass(m) => assert!(!m.is_abstract),
        other => panic!("expected mixin class, got {other:?}"),
    }
}

#[test]
fn test_type_param_annotation() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C<@foo T> {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let c = only_class(&prog);
    assert_eq!(c.type_params.len(), 1);
    assert_eq!(c.type_params[0].annotations.len(), 1);
    assert_eq!(c.type_params[0].annotations[0].name[0].name, "foo");
}

#[test]
fn test_annotation_dot_new_reference() {
    // `@X.new()` — a metadata reference to the unnamed constructor. `new` is a
    // keyword token, so the annotation name parser must accept it.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("@X.new() class C {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let c = only_class(&prog);
    let ann = &c.annotations[0];
    assert_eq!(ann.name.last().unwrap().name, "new");
    assert!(ann.args.is_some(), "the `()` is an argument list");
}

#[test]
fn test_annotation_arg_const_dot_new() {
    // `@Helper(const C.new())` — a const object creation using `.new` inside the
    // annotation's argument list.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("@Helper(const C.new()) class D {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let ann = &only_class(&prog).annotations[0];
    assert_eq!(ann.name[0].name, "Helper");
    let arg = &ann.args.as_ref().unwrap().positional[0];
    match arg {
        Expr::New {
            is_const,
            constructor_name,
            ..
        } => {
            assert!(is_const);
            assert_eq!(constructor_name.as_ref().unwrap().name, "new");
        }
        other => panic!("expected const `C.new()`, got {other:?}"),
    }
}

#[test]
fn test_curried_function_type_param() {
    // `int Function() Function() x` — a function returning `int Function()`.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("foo(int Function() Function() x) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    };
    let param = &func.params().unwrap().params[0];
    assert_eq!(param.kind, ParamKind::Positional);
    assert_eq!(param.name.name, "x");
    match param.param_type.as_ref().unwrap() {
        DartType::Function(outer) => {
            // The outer function type's return type is another function type.
            match outer.return_type.as_deref() {
                Some(DartType::Function(inner)) => {
                    assert!(
                        matches!(inner.return_type.as_deref(), Some(DartType::Named(_))),
                        "innermost return should be `int`"
                    );
                }
                other => panic!("expected inner function-type return, got {other:?}"),
            }
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn test_curried_function_type_in_type_args() {
    // `@G<void Function() Function(), int>()` — a curried function type as a
    // type argument alongside a plain type.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("@G<void Function() Function(), int>() class E {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let ann = &only_class(&prog).annotations[0];
    assert_eq!(ann.type_args.len(), 2);
    match &ann.type_args[0] {
        DartType::Function(outer) => assert!(matches!(
            outer.return_type.as_deref(),
            Some(DartType::Function(_))
        )),
        other => panic!("expected curried function type, got {other:?}"),
    }
    assert!(matches!(ann.type_args[1], DartType::Named(_)));
}

#[test]
fn test_top_level_untyped_getter_arrow() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("get foo => 1;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => {
            assert!(matches!(f.kind, falcon_syntax::ast::FunctionKind::Getter));
            assert!(f.params().is_none());
            assert_eq!(f.name.name, "foo");
            assert!(f.return_type.is_none());
            assert!(matches!(f.body, Some(FunctionBody::Arrow(..))));
        }
        other => panic!("expected getter function, got {other:?}"),
    }
}

#[test]
fn test_top_level_untyped_getter_block() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("get foo {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => {
            assert!(matches!(f.kind, falcon_syntax::ast::FunctionKind::Getter));
            assert!(f.params().is_none());
            assert_eq!(f.name.name, "foo");
            assert!(f.return_type.is_none());
            assert!(matches!(f.body, Some(FunctionBody::Block(_))));
        }
        other => panic!("expected getter function, got {other:?}"),
    }
}

#[test]
fn test_top_level_typed_getter_still_parses() {
    // Control: a getter WITH a return type keeps the type.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("int get foo => 1;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => {
            assert!(matches!(f.kind, falcon_syntax::ast::FunctionKind::Getter));
            assert!(f.params().is_none());
            assert!(matches!(f.return_type, Some(DartType::Named(_))));
        }
        other => panic!("expected getter function, got {other:?}"),
    }
}

#[test]
fn test_static_modifier_at_top_level_rejected() {
    // `static` on a top-level declaration is an extraneous modifier the SDK
    // reports; falcon used to silently accept it. It still recovers and parses
    // the underlying variable.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("static final clientInfo = 3;");
    assert_eq!(errors.len(), 1, "errors: {errors:?}");
    assert!(errors[0].message.contains("static"));
    assert!(matches!(&prog.declarations[0], TopLevelDecl::Variable(_)));
}

#[test]
fn test_top_level_function_named_static_ok() {
    // Control: a top-level function *named* `static` (a `(`/`<` follows) is legal.
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("static() => 1;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => assert_eq!(f.name.name, "static"),
        other => panic!("expected function named `static`, got {other:?}"),
    }
}

#[test]
fn test_missing_semicolon_after_arrow_switch_body_rejected() {
    // An `=> switch (a) {...}` function body with no trailing `;` must ERROR;
    // falcon used to silently recover the missing terminator.
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("int f(Object a) => switch (a) { int x => 0, _ => 1 }");
    assert!(!errors.is_empty(), "expected a missing-semicolon error");
    assert!(errors.iter().any(|e| e.message.contains("Semicolon")));
}

#[test]
fn test_arrow_switch_body_with_semicolon_ok() {
    // Control: the terminated form parses clean.
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("int f(Object a) => switch (a) { int x => 0, _ => 1 };");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

fn for_init_decl(src: &str) -> LocalVarDecl {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f.body.as_ref().expect("function body"),
        other => panic!("expected function, got {other:?}"),
    };
    let stmts = match body {
        FunctionBody::Block(b) => &b.stmts,
        other => panic!("expected block body, got {other:?}"),
    };
    match &stmts[0] {
        Stmt::For(f) => match f.header.init.as_ref().expect("for init") {
            ForInit::VarDecl(lv) => lv.clone(),
            other => panic!("expected var-decl init, got {other:?}"),
        },
        other => panic!("expected for statement, got {other:?}"),
    }
}

#[test]
fn test_const_in_c_style_for_initializer_keeps_keyword() {
    let decl = for_init_decl("void f() { for (const kX = 1; kX < 2;) {} }");
    assert_eq!(decl.keyword, VarKeyword::Const);
    assert_eq!(decl.declarators[0].name.name, "kX");
}

#[test]
fn test_final_in_c_style_for_initializer_keeps_keyword() {
    let decl = for_init_decl("void f() { for (final x = 1; x < 2;) {} }");
    assert_eq!(decl.keyword, VarKeyword::Final);
}

#[test]
fn test_var_in_c_style_for_initializer_keeps_keyword() {
    let decl = for_init_decl("void f() { for (var i = 0; i < 2; i++) {} }");
    assert_eq!(decl.keyword, VarKeyword::Var);
}

#[test]
fn test_getter_preserves_all_body_markers() {
    for (source_marker, expected) in [
        ("", AsyncMarker::Sync),
        ("sync*", AsyncMarker::SyncStar),
        ("async", AsyncMarker::Async),
        ("async*", AsyncMarker::AsyncStar),
    ] {
        let falcon_dart_parser::ParseResult {
            program: prog,
            errors,
            ..
        } = parse(&format!("class C {{ get values {source_marker} {{}} }}"));
        assert!(errors.is_empty(), "errors: {errors:?}");
        let ClassMember::Getter(getter) = &only_class(&prog).members[0] else {
            panic!("expected getter");
        };
        assert_eq!(getter.async_marker, expected);
        assert_eq!(expected.is_async(), source_marker.starts_with("async"));
        assert_eq!(expected.is_generator(), source_marker.ends_with('*'));
    }
}

#[test]
fn test_body_less_accessors_are_abstract_unless_external() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(
        "abstract class C { int get a; set a(int v); int get b => 0; set b(int v) {} external int get c; external set c(int v); }",
    );
    assert!(errors.is_empty(), "errors: {errors:?}");
    let abstractness: Vec<_> = only_class(&prog)
        .members
        .iter()
        .map(|member| match member {
            ClassMember::Getter(getter) => getter.is_abstract,
            ClassMember::Setter(setter) => setter.is_abstract,
            other => panic!("expected accessor, got {other:?}"),
        })
        .collect();
    assert_eq!(abstractness, [true, true, false, false, false, false]);
}

#[test]
fn test_for_in_preserves_variable_keyword() {
    for (source_keyword, expected) in [
        ("var", VarKeyword::Var),
        ("final", VarKeyword::Final),
        ("const", VarKeyword::Const),
    ] {
        let falcon_dart_parser::ParseResult {
            program: prog,
            errors,
            ..
        } = parse(&format!(
            "void f() {{ for ({source_keyword} x in xs) {{}} }}"
        ));
        assert!(errors.is_empty(), "errors: {errors:?}");
        let TopLevelDecl::Function(function) = &prog.declarations[0] else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = &function.body else {
            panic!("expected function body");
        };
        let Stmt::For(statement) = &body.stmts[0] else {
            panic!("expected for statement");
        };
        let Some(ForInit::ForIn {
            keyword,
            declares_variable,
            ..
        }) = &statement.header.init
        else {
            panic!("expected for-in header");
        };
        assert_eq!(*keyword, expected);
        assert!(*declares_variable);
    }
}

#[test]
fn test_for_in_distinguishes_assignment_from_declaration() {
    for (header, expected) in [("x", false), ("var x", true), ("int x", true)] {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(&format!("void f() {{ for ({header} in xs) {{}} }}"));
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = &function.body else {
            panic!("expected block");
        };
        let Stmt::For(statement) = &body.stmts[0] else {
            panic!("expected for");
        };
        let Some(ForInit::ForIn {
            declares_variable, ..
        }) = &statement.header.init
        else {
            panic!("expected for-in");
        };
        assert_eq!(*declares_variable, expected, "{header}");
    }
}

#[test]
fn test_pattern_for_in_preserves_variable_keyword() {
    for (source_keyword, expected) in [("var", VarKeyword::Var), ("final", VarKeyword::Final)] {
        let falcon_dart_parser::ParseResult {
            program: prog,
            errors,
            ..
        } = parse(&format!(
            "void f() {{ for ({source_keyword} (x, y) in pairs) {{}} }}"
        ));
        assert!(errors.is_empty(), "errors: {errors:?}");
        let TopLevelDecl::Function(function) = &prog.declarations[0] else {
            panic!("expected function");
        };
        let Some(FunctionBody::Block(body)) = &function.body else {
            panic!("expected function body");
        };
        let Stmt::For(statement) = &body.stmts[0] else {
            panic!("expected for statement");
        };
        let Some(ForInit::PatternForIn { keyword, .. }) = &statement.header.init else {
            panic!("expected pattern for-in header");
        };
        assert_eq!(*keyword, expected);
    }
}

// ── Metadata annotations on record-type fields ─────────────────────────────────

#[test]
fn test_metadata_on_positional_record_type_field() {
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("const meta = 0;\n(int, @meta int) r = (1, 2);");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_metadata_on_named_record_type_field() {
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("const meta = 0;\n({int a, @meta int b}) r = (a: 1, b: 2);");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_metadata_on_record_type_field_in_param() {
    let falcon_dart_parser::ParseResult {
        program: _prog,
        errors,
        ..
    } = parse("const meta = 0;\nvoid f((int, @meta String) r) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_operator_consumes_body_markers() {
    for marker in ["async", "async*", "sync*"] {
        let statement = if marker.ends_with('*') {
            "yield o;"
        } else {
            "await o;"
        };
        let source = format!("class C {{ operator +(o) {marker} {{ {statement} }} }}");
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(&source);
        assert!(errors.is_empty(), "{marker}: {errors:?}");
        let ClassMember::Operator(operator) = &only_class(&program).members[0] else {
            panic!("expected operator");
        };
        let Some(FunctionBody::Block(block)) = &operator.body else {
            panic!("expected operator block");
        };
        assert_eq!(block.stmts.len(), 1);
        if marker.ends_with('*') {
            assert!(matches!(block.stmts[0], Stmt::Yield(_)));
        } else {
            assert!(matches!(
                &block.stmts[0],
                Stmt::Expr(ExprStmt {
                    expr: Expr::Await { .. },
                    ..
                })
            ));
        }
        assert_eq!(
            &source[operator.span.start..operator.span.end],
            &source[10..source.len() - 2]
        );
    }
}

#[test]
fn test_declarator_spans_start_at_each_name() {
    for source in [
        "int first = 1, second = 2;",
        "final first = 1, second = 2;",
        "class C { int first = 1, second = 2; }",
        "class C { final first = 1, second = 2; }",
    ] {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
        let declarators = match &program.declarations[0] {
            TopLevelDecl::Variable(variable) => &variable.declarators,
            TopLevelDecl::Class(class) => {
                let ClassMember::Field(field) = &class.members[0] else {
                    panic!("expected field");
                };
                &field.declarators
            }
            other => panic!("expected variable or class, got {other:?}"),
        };
        assert_eq!(declarators.len(), 2);
        for (declarator, expected) in declarators.iter().zip(["first = 1", "second = 2"]) {
            assert_eq!(declarator.span.start, declarator.name.span.start);
            assert_eq!(
                &source[declarator.span.start..declarator.span.end],
                expected
            );
        }
    }
}

#[test]
fn test_primary_constructor_class() {
    let source = "class const C.named(var int x, final y, z) { @deprecated this : assert(z > 0); }";
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let class = only_class(&prog);
    let primary = class
        .primary_constructor
        .as_ref()
        .expect("primary constructor");
    assert!(primary.is_const);
    assert_eq!(
        primary.constructor_name.as_ref().map(|n| n.name.as_str()),
        Some("named")
    );
    let declaring: Vec<_> = class
        .element_members()
        .declaring_params()
        .map(|p| p.name.name.as_str())
        .collect();
    assert_eq!(declaring, ["x", "y"]);
    assert!(primary.params.params[0].param_type.is_some());
    assert!(matches!(
        class.members[0],
        ClassMember::PrimaryConstructorBody(_)
    ));
    let ClassMember::PrimaryConstructorBody(body) = &class.members[0] else {
        unreachable!()
    };
    assert_eq!(&source[body.this_span.start..body.this_span.end], "this");
    assert!(body.span.start < body.this_span.start);
    let members = class.element_members();
    let fields = members.declaring_params().count();
    let ctors = members.constructors().count();
    assert_eq!((fields, ctors), (2, 1));
}

#[test]
fn primary_constructor_declared_body_name_excludes_annotations() {
    let source = "class C() { @deprecated this {} }";
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let mut names = Vec::new();
    falcon_syntax::visitor::for_each_declared_body(&program, source, &mut |_, name, _| {
        names.push(&source[name.start..name.end]);
    });
    assert_eq!(names, ["this"]);
}

#[test]
fn test_primary_constructor_enum_and_empty_body() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("enum E(final int x) { a(1); }\nclass D(dynamic a);");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(&prog.declarations[0], TopLevelDecl::Enum(e) if e.primary_constructor.is_some())
    );
    assert!(
        matches!(&prog.declarations[1], TopLevelDecl::Class(c) if c.primary_constructor.is_some() && c.members.is_empty())
    );
}

#[test]
fn test_function_named_old_style_formal() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("class C { void map(int Function(dynamic p)) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            let p = &m.params.params[0];
            assert_eq!(p.name.name, "Function");
            assert!(p.function_params.is_some());
            assert!(p.param_type.is_some());
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_optional_group_must_be_last() {
    let falcon_dart_parser::ParseResult { errors, .. } = parse("void f3(x, [y], {z}) {}");
    assert!(!errors.is_empty());
}

#[test]
fn test_legacy_typedef_keeps_formals() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("typedef String Type2(final value);");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::TypeAlias(a) => {
            let params = a.legacy_params.as_ref().expect("legacy params");
            assert!(params.params[0].is_final);
            assert!(params.params[0].param_type.is_none());
        }
        other => panic!("expected typedef, got {other:?}"),
    }
}

#[test]
fn test_factory_paren_is_method_before_3_13() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("// @dart = 3.12\nclass B { factory (A) => B._(); B._(); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(
        only_class(&prog).members[0],
        ClassMember::Method(_)
    ));
    let falcon_dart_parser::ParseResult { program: prog, .. } =
        parse("class B { factory () => B._(); B._(); }");
    assert!(matches!(
        only_class(&prog).members[0],
        ClassMember::Constructor(_)
    ));
}

#[test]
fn test_pre_patterns_case_is_expression() {
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse("// @dart = 2.19\nvoid f(Object o) { switch (o) { case 1 + 2: break; } }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(f) = &prog.declarations[0] else {
        panic!()
    };
    let Some(FunctionBody::Block(b)) = &f.body else {
        panic!()
    };
    let Stmt::Switch(s) = &b.stmts[0] else {
        panic!()
    };
    assert!(matches!(s.cases[0].cases[0], SwitchCaseKind::Expr(_)));
}
