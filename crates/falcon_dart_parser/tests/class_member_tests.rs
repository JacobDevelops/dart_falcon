//! Regression tests for the class-member classification mangle: an untyped
//! `name(params) {...}` member is a constructor ONLY when its name (before any
//! `.`) equals the enclosing type name. Any other untyped member is a method
//! with no return type. Mixins and plain extensions cannot declare constructors,
//! so their untyped members are always methods. Each case must parse with zero
//! errors and produce the faithful AST shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

/// Parse whole-program `src` and return its parse-error count.
fn errs(src: &str) -> usize {
    parse(src).1.len()
}

fn only_class(prog: &Program) -> &ClassDecl {
    match &prog.declarations[0] {
        TopLevelDecl::Class(c) => c,
        other => panic!("expected class, got {other:?}"),
    }
}

fn assert_clean(errors: &[falcon_dart_parser::parser::ParseError]) {
    assert!(
        errors.is_empty(),
        "expected no parse errors, got: {errors:?}"
    );
}

// ── Untyped members whose name != enclosing type are methods ───────────────────

#[test]
fn test_untyped_member_is_method_not_constructor() {
    let (prog, errors) = parse("class C { foo() {} }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "foo");
            assert!(m.return_type.is_none());
            assert!(!m.is_async);
            assert!(!m.is_generator);
            assert!(m.type_params.is_empty());
            assert!(m.body.is_some());
        }
        other => panic!("expected method `foo`, got {other:?}"),
    }
}

#[test]
fn test_test_prefixed_member_is_method() {
    // The ubiquitous real-world case: `test_x() {}` in a test class.
    let (prog, errors) = parse("class MyTest { test_login() {} test_logout() {} }");
    assert_clean(&errors);
    let c = only_class(&prog);
    for m in &c.members {
        match m {
            ClassMember::Method(_) => {}
            other => panic!("expected method, got {other:?}"),
        }
    }
}

#[test]
fn test_untyped_member_with_body_and_params_is_method() {
    let (prog, errors) = parse("class C { doWork(int a, int b) { return; } }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "doWork");
            assert!(m.return_type.is_none());
            assert_eq!(m.params.positional.len(), 2);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_abstract_member_is_method() {
    // No body → abstract method, still classified as a method (name != type).
    let (prog, errors) = parse("class C { foo(); }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "foo");
            assert!(m.return_type.is_none());
            assert!(m.is_abstract);
            assert!(m.body.is_none());
        }
        other => panic!("expected method, got {other:?}"),
    }
}

// ── Real constructors still classify as constructors ───────────────────────────

#[test]
fn test_unnamed_constructor_still_constructor() {
    let (prog, errors) = parse("class C { C() {} }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(ctor) => {
            assert_eq!(ctor.name.name, "C");
            assert!(ctor.constructor_name.is_none());
            assert!(!ctor.is_factory);
            assert!(!ctor.is_const);
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_named_constructor_still_constructor() {
    let (prog, errors) = parse("class C { C.named() {} }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(ctor) => {
            assert_eq!(ctor.name.name, "C");
            assert_eq!(
                ctor.constructor_name.as_ref().map(|n| n.name.as_str()),
                Some("named")
            );
        }
        other => panic!("expected named constructor, got {other:?}"),
    }
}

#[test]
fn test_unnamed_and_named_constructor_with_initializers() {
    let (prog, errors) = parse("class C { int x; C(this.x); C.zero() : x = 0; }");
    assert_clean(&errors);
    let c = only_class(&prog);
    assert!(matches!(&c.members[1], ClassMember::Constructor(_)));
    assert!(matches!(&c.members[2], ClassMember::Constructor(_)));
}

#[test]
fn test_const_constructor_still_constructor() {
    let (prog, errors) = parse("class C { const C(); }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(ctor) => {
            assert_eq!(ctor.name.name, "C");
            assert!(ctor.is_const);
        }
        other => panic!("expected const constructor, got {other:?}"),
    }
}

#[test]
fn test_factory_constructor_still_constructor() {
    let (prog, errors) = parse("class C { factory C.make() => C._(); C._(); }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Constructor(ctor) => {
            assert!(ctor.is_factory);
            assert_eq!(
                ctor.constructor_name.as_ref().map(|n| n.name.as_str()),
                Some("make")
            );
        }
        other => panic!("expected factory constructor, got {other:?}"),
    }
}

// ── Generic / async untyped members named like the type stay methods ───────────

#[test]
fn test_generic_untyped_member_named_like_type_is_method() {
    // A constructor cannot declare type parameters, so `C<T>()` is a method.
    let (prog, errors) = parse("class C { C<T>() {} }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "C");
            assert!(m.return_type.is_none());
            assert_eq!(m.type_params.len(), 1);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_async_untyped_member_named_like_type_is_method() {
    // A constructor cannot be async, so `C() async {}` is a method.
    let (prog, errors) = parse("class C { C() async {} }");
    assert_clean(&errors);
    match &only_class(&prog).members[0] {
        ClassMember::Method(m) => {
            assert_eq!(m.name.name, "C");
            assert!(m.is_async);
            assert!(m.return_type.is_none());
        }
        other => panic!("expected async method, got {other:?}"),
    }
}

// ── Enums admit constructors ───────────────────────────────────────────────────

#[test]
fn test_enum_untyped_member_is_method() {
    let (prog, errors) = parse("enum E { a, b; foo() {} }");
    assert_clean(&errors);
    match &prog.declarations[0] {
        TopLevelDecl::Enum(e) => match &e.members[0] {
            ClassMember::Method(m) => {
                assert_eq!(m.name.name, "foo");
                assert!(m.return_type.is_none());
            }
            other => panic!("expected method in enum, got {other:?}"),
        },
        other => panic!("expected enum, got {other:?}"),
    }
}

#[test]
fn test_enum_constructor_is_constructor() {
    let (prog, errors) = parse("enum E { a(1), b(2); final int n; const E(this.n); }");
    assert_clean(&errors);
    match &prog.declarations[0] {
        TopLevelDecl::Enum(e) => {
            let ctor = e
                .members
                .iter()
                .find_map(|m| match m {
                    ClassMember::Constructor(c) => Some(c),
                    _ => None,
                })
                .expect("expected a constructor member in the enum");
            assert_eq!(ctor.name.name, "E");
            assert!(ctor.is_const);
        }
        other => panic!("expected enum, got {other:?}"),
    }
}

// ── Mixins and extensions cannot declare constructors ──────────────────────────

#[test]
fn test_mixin_untyped_member_is_method() {
    // Even a member named after the mixin is a method — mixins have no ctors.
    let (prog, errors) = parse("mixin M { M() {} foo() {} }");
    assert_clean(&errors);
    match &prog.declarations[0] {
        TopLevelDecl::Mixin(m) => {
            for member in &m.members {
                match member {
                    ClassMember::Method(_) => {}
                    other => panic!("expected method in mixin, got {other:?}"),
                }
            }
        }
        other => panic!("expected mixin, got {other:?}"),
    }
}

#[test]
fn test_extension_untyped_member_is_method() {
    let (prog, errors) = parse("extension E on int { foo() {} }");
    assert_clean(&errors);
    match &prog.declarations[0] {
        TopLevelDecl::Extension(e) => match &e.members[0] {
            ClassMember::Method(m) => {
                assert_eq!(m.name.name, "foo");
                assert!(m.return_type.is_none());
            }
            other => panic!("expected method in extension, got {other:?}"),
        },
        other => panic!("expected extension, got {other:?}"),
    }
}

// ── Extension types admit constructors ─────────────────────────────────────────

#[test]
fn test_extension_type_untyped_member_is_method() {
    let (prog, errors) = parse("extension type E(int it) { foo() {} }");
    assert_clean(&errors);
    match &prog.declarations[0] {
        TopLevelDecl::ExtensionType(e) => match &e.members[0] {
            ClassMember::Method(m) => {
                assert_eq!(m.name.name, "foo");
                assert!(m.return_type.is_none());
            }
            other => panic!("expected method in extension type, got {other:?}"),
        },
        other => panic!("expected extension type, got {other:?}"),
    }
}

#[test]
fn test_extension_type_named_constructor_is_constructor() {
    let (prog, errors) = parse("extension type E(int it) { E.zero() : it = 0; }");
    assert_clean(&errors);
    match &prog.declarations[0] {
        TopLevelDecl::ExtensionType(e) => match &e.members[0] {
            ClassMember::Constructor(ctor) => {
                assert_eq!(ctor.name.name, "E");
                assert_eq!(
                    ctor.constructor_name.as_ref().map(|n| n.name.as_str()),
                    Some("zero")
                );
            }
            other => panic!("expected constructor in extension type, got {other:?}"),
        },
        other => panic!("expected extension type, got {other:?}"),
    }
}

// ── Corpus-found class-member gaps ───────────────────────────────────────

#[test]
fn annotation_then_named_field_record_return() {
    assert_eq!(errs("class C { @override ({int a, int b})? m() {} }"), 0);
}

#[test]
fn annotation_with_real_args_still_parses() {
    // Regression guard: a genuine annotation argument list is untouched.
    let src = "class C { @Foo({'a': 1}) int m() => 0; }";
    assert_eq!(errs(src), 0);
}

#[test]
fn arrow_closure_field_initializer() {
    assert_eq!(errs("class C { final f = () => 0; }"), 0);
}

#[test]
fn arrow_closure_field_then_more_members() {
    // The `;` must reach the field parser so the next member parses cleanly.
    assert_eq!(errs("class C { final f = () => 0; final g = 1; }"), 0);
}

#[test]
fn enum_constant_named_constructor() {
    let src = "enum E { a.foo(1); const E.foo(this.n); final int n; }";
    let (prog, errors) = parse(src);
    assert_eq!(errors.len(), 0, "errors: {errors:?}");
    let en = match &prog.declarations[0] {
        TopLevelDecl::Enum(e) => e,
        other => panic!("expected enum, got {other:?}"),
    };
    let ctor = en.variants[0].constructor_name.as_ref();
    assert_eq!(ctor.map(|i| i.name.as_str()), Some("foo"));
}

#[test]
fn setter_trailing_comma() {
    assert_eq!(errs("class C { set x(int a,) {} }"), 0);
}

#[test]
fn setter_final_param() {
    assert_eq!(errs("class C { set x(final Level a) {} }"), 0);
}

#[test]
fn field_named_operator() {
    // `operator` is a built-in identifier: a real member name when not followed by
    // an operator symbol.
    assert_eq!(errs("class C { Token? operator; }"), 0);
}

#[test]
fn operator_overload_still_parses() {
    let src = "class C { int operator +(int o) => 0; C operator [](int i) => this; }";
    assert_eq!(errs(src), 0);
}

#[test]
fn modifier_keyword_static_used_as_field_name() {
    assert_eq!(errs("class C { static const static = 1; }"), 0);
}
