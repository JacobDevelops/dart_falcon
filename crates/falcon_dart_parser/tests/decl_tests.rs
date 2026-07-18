//! Regression tests for declaration-level parser gaps (batch 1): each construct
//! must parse with zero errors and produce a faithful AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

fn only_class(prog: &Program) -> &ClassDecl {
    match &prog.declarations[0] {
        TopLevelDecl::Class(c) => c,
        other => panic!("expected class, got {other:?}"),
    }
}

// ── Item 1: untyped method with async/generator modifier ──────────────────────

#[test]
fn test_untyped_async_method() {
    let (prog, errors) = parse("class C { foo() async {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let c = only_class(&prog);
    match &c.members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "foo");
            assert!(m.is_async);
            assert!(!m.is_generator);
            assert!(m.return_type.is_none());
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_async_star_method() {
    let (prog, errors) = parse("class C { foo() async* {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert!(m.is_async);
            assert!(m.is_generator);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_sync_star_method() {
    let (prog, errors) = parse("class C { foo() sync* {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert!(!m.is_async);
            assert!(m.is_generator);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_generic_async_method() {
    let (prog, errors) = parse("class C { foo<T>() async {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.type_params.len(), 1);
            assert!(m.is_async);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

// ── Item 1 disambiguation: async/sync are not reserved ────────────────────────

#[test]
fn test_field_named_async_still_parses() {
    let (prog, errors) = parse("class C { var async = 1; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Field(f) => assert_eq!(f.declarators[0].name.name, "async"),
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn test_method_named_async_still_parses() {
    // A member named `async` parses cleanly (as an untyped no-arg member).
    let (prog, errors) = parse("class C { async() {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let name = match &only_class(&prog).members[0] {
        ClassMember::Constructor(c) => c.name.name.clone(),
        ClassMember::Method(m) => m.name.name.clone(),
        other => panic!("unexpected member {other:?}"),
    };
    assert_eq!(name, "async");
}

#[test]
fn test_getter_named_sync_still_parses() {
    let (prog, errors) = parse("class C { int get sync => 1; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Getter(g) => assert_eq!(g.name.name, "sync"),
        other => panic!("expected getter, got {other:?}"),
    }
}

// ── Item 2: operator [] and []= ───────────────────────────────────────────────

#[test]
fn test_index_operator() {
    let (prog, errors) = parse("class C { int operator [](int i) => 0; }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Operator(op) => assert_eq!(op.op, "[]"),
        other => panic!("expected operator, got {other:?}"),
    }
}

#[test]
fn test_index_assign_operator() {
    let (prog, errors) = parse("class C { void operator []=(int i, int v) {} }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Operator(op) => assert_eq!(op.op, "[]="),
        other => panic!("expected operator, got {other:?}"),
    }
}

// ── Item 3: extension type named/const constructor ────────────────────────────

#[test]
fn test_extension_type_const_named_ctor() {
    let (prog, errors) = parse("extension type const Foo._(int x) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::ExtensionType(e) => {
            assert!(e.is_const);
            assert_eq!(e.name.name, "Foo");
            assert_eq!(
                e.representation
                    .constructor_name
                    .as_ref()
                    .map(|i| i.name.as_str()),
                Some("_")
            );
            assert_eq!(e.representation.field_name.name, "x");
        }
        other => panic!("expected extension type, got {other:?}"),
    }
}

// ── Item 4: redirecting factory ───────────────────────────────────────────────

#[test]
fn test_redirecting_factory() {
    let (prog, errors) = parse("class C { factory C() = D; }");
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
    let (prog, errors) = parse("class C { factory C() = D<int>; }");
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

// ── Item 5: typed super-parameter ─────────────────────────────────────────────

#[test]
fn test_typed_super_parameter() {
    let (prog, errors) = parse("class C extends B { C(int super.x); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(c) => {
            let p = &c.params.positional[0];
            assert!(p.is_super);
            assert_eq!(p.name.name, "x");
            assert!(p.param_type.is_some());
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

// ── Item 6: conditional import ────────────────────────────────────────────────

#[test]
fn test_conditional_import() {
    let (prog, errors) = parse("import 'a.dart' if (dart.library.io) 'b.dart';");
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
    let (prog, errors) = parse("export 'a.dart' if (dart.library.io == 'true') 'b.dart';");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let cu = &prog.exports[0].configurable_uris[0];
    assert_eq!(cu.value.as_ref().map(|s| s.value.as_str()), Some("true"));
}

// ── Item 7: annotation with type arguments ────────────────────────────────────

#[test]
fn test_annotation_with_type_args() {
    let (prog, errors) = parse("@Native<int Function()>() external void f();");
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

// ── Item 8: typed field formal ────────────────────────────────────────────────

#[test]
fn test_typed_field_formal() {
    let (prog, errors) = parse("class C { int x; C(int this.x); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &only_class(&prog).members[1] {
        ClassMember::Constructor(c) => {
            let p = &c.params.positional[0];
            assert!(p.is_field);
            assert_eq!(p.name.name, "x");
            assert!(p.param_type.is_some());
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

// ── Item 9: mixin-application class ───────────────────────────────────────────

#[test]
fn test_mixin_application_class() {
    let (prog, errors) = parse("class MA = S with M;");
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
    let (prog, errors) = parse("abstract class MA = S with M implements I;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::ClassTypeAlias(a) => {
            assert!(a.modifiers.is_abstract);
            assert_eq!(a.implements.len(), 1);
        }
        other => panic!("expected class type alias, got {other:?}"),
    }
}

// ── Item 10: old-form typedef ─────────────────────────────────────────────────

#[test]
fn test_legacy_typedef() {
    let (prog, errors) = parse("typedef int Comparator(int a, int b);");
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
    let (prog, errors) = parse("typedef Comparator(int a, int b);");
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

// ── Item 11: nullable old-style function-typed formal ─────────────────────────

#[test]
fn test_nullable_function_typed_formal() {
    let (prog, errors) = parse("void f({int orElse()?}) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(func) => {
            let p = &func.params.named[0];
            assert_eq!(p.name.name, "orElse");
            assert!(p.function_params.is_some());
        }
        other => panic!("expected function, got {other:?}"),
    }
}

// ── Item 12a: mixin class real abstract modifier ──────────────────────────────

#[test]
fn test_abstract_mixin_class() {
    let (prog, errors) = parse("abstract mixin class M {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::MixinClass(m) => assert!(m.is_abstract),
        other => panic!("expected mixin class, got {other:?}"),
    }
}

#[test]
fn test_non_abstract_mixin_class() {
    let (prog, errors) = parse("mixin class M {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::MixinClass(m) => assert!(!m.is_abstract),
        other => panic!("expected mixin class, got {other:?}"),
    }
}

// ── Item 12b: type-parameter annotation ───────────────────────────────────────

#[test]
fn test_type_param_annotation() {
    let (prog, errors) = parse("class C<@foo T> {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let c = only_class(&prog);
    assert_eq!(c.type_params.len(), 1);
    assert_eq!(c.type_params[0].annotations.len(), 1);
    assert_eq!(c.type_params[0].annotations[0].name[0].name, "foo");
}

// ── Item: `.new` named-constructor references in annotations ───────────────────

#[test]
fn test_annotation_dot_new_reference() {
    // `@X.new()` — a metadata reference to the unnamed constructor. `new` is a
    // keyword token, so the annotation name parser must accept it.
    let (prog, errors) = parse("@X.new() class C {}");
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
    let (prog, errors) = parse("@Helper(const C.new()) class D {}");
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

// ── Item: curried function types (return type is itself a function type) ───────

#[test]
fn test_curried_function_type_param() {
    // `int Function() Function() x` — a function returning `int Function()`.
    let (prog, errors) = parse("foo(int Function() Function() x) {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let func = match &prog.declarations[0] {
        TopLevelDecl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    };
    let param = &func.params.positional[0];
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
    let (prog, errors) = parse("@G<void Function() Function(), int>() class E {}");
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

// ── Item: top-level untyped getters (`get foo => 1;`, `get foo {}`) ────────────

#[test]
fn test_top_level_untyped_getter_arrow() {
    let (prog, errors) = parse("get foo => 1;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => {
            assert!(f.is_getter);
            assert_eq!(f.name.name, "foo");
            assert!(f.return_type.is_none());
            assert!(matches!(f.body, Some(FunctionBody::Arrow(..))));
        }
        other => panic!("expected getter function, got {other:?}"),
    }
}

#[test]
fn test_top_level_untyped_getter_block() {
    let (prog, errors) = parse("get foo {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => {
            assert!(f.is_getter);
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
    let (prog, errors) = parse("int get foo => 1;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => {
            assert!(f.is_getter);
            assert!(matches!(f.return_type, Some(DartType::Named(_))));
        }
        other => panic!("expected getter function, got {other:?}"),
    }
}

// ── `static` is only legal on class members, not at top level ──────────────────

#[test]
fn test_static_modifier_at_top_level_rejected() {
    // `static` on a top-level declaration is an extraneous modifier the SDK
    // reports; falcon used to silently accept it. It still recovers and parses
    // the underlying variable.
    let (prog, errors) = parse("static final clientInfo = 3;");
    assert_eq!(errors.len(), 1, "errors: {errors:?}");
    assert!(errors[0].message.contains("static"));
    assert!(matches!(&prog.declarations[0], TopLevelDecl::Variable(_)));
}

#[test]
fn test_top_level_function_named_static_ok() {
    // Control: a top-level function *named* `static` (a `(`/`<` follows) is legal.
    let (prog, errors) = parse("static() => 1;");
    assert!(errors.is_empty(), "errors: {errors:?}");
    match &prog.declarations[0] {
        TopLevelDecl::Function(f) => assert_eq!(f.name.name, "static"),
        other => panic!("expected function named `static`, got {other:?}"),
    }
}

// ── Missing `;` after an `=> switch(...) {...}` body is a syntax error ──────────

#[test]
fn test_missing_semicolon_after_arrow_switch_body_rejected() {
    // An `=> switch (a) {...}` function body with no trailing `;` must ERROR;
    // falcon used to silently recover the missing terminator.
    let (_prog, errors) = parse("int f(Object a) => switch (a) { int x => 0, _ => 1 }");
    assert!(!errors.is_empty(), "expected a missing-semicolon error");
    assert!(errors.iter().any(|e| e.message.contains("Semicolon")));
}

#[test]
fn test_arrow_switch_body_with_semicolon_ok() {
    // Control: the terminated form parses clean.
    let (_prog, errors) = parse("int f(Object a) => switch (a) { int x => 0, _ => 1 };");
    assert!(errors.is_empty(), "errors: {errors:?}");
}
