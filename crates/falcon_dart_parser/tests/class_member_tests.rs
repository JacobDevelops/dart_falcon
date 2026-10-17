//! Regression tests for the class-member classification mangle: an untyped
//! `name(params) {...}` member is a constructor ONLY when its name (before any
//! `.`) equals the enclosing type name. Any other untyped member is a method
//! with no return type. Mixins and plain extensions cannot declare constructors,
//! so their untyped members are always methods. Each case must parse with zero
//! errors and produce the faithful CST view shape.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;

#[path = "common/only_class.rs"]
mod common_class;
#[path = "common/errs.rs"]
mod common_errs;
use common_class::only_class;
use common_errs::errs;

fn assert_clean(errors: &[falcon_dart_parser::parser::ParseError]) {
    assert!(
        errors.is_empty(),
        "expected no parse errors, got: {errors:?}"
    );
}

#[test]
fn test_untyped_member_is_method_not_constructor() {
    let source = "class C { foo() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "foo");
            assert!(m.return_type().is_none());
            assert_eq!(m.async_marker(), AsyncMarker::Sync);
            assert!(m.type_params().is_empty());
            assert!(m.body().is_some());
        }
        other => panic!("expected method `foo`, got {other:?}"),
    }
}

#[test]
fn test_test_prefixed_member_is_method() {
    // The ubiquitous real-world case: `test_x() {}` in a test class.
    let source = "class MyTest { test_login() {} test_logout() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let c = only_class(prog);
    for m in c.members() {
        match m {
            ClassMember::Method(_) => {}
            other => panic!("expected method, got {other:?}"),
        }
    }
}

#[test]
fn test_untyped_member_with_body_and_params_is_method() {
    let source = "class C { doWork(int a, int b) { return; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "doWork");
            assert!(m.return_type().is_none());
            assert_eq!(m.params().params().len(), 2);
            assert!(
                m.params()
                    .params()
                    .all(|param| param.kind() == ParamKind::Positional)
            );
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_untyped_abstract_member_is_method() {
    // No body → abstract method, still classified as a method (name != type).
    let source = "class C { foo(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "foo");
            assert!(m.return_type().is_none());
            assert!(m.is_abstract());
            assert!(m.body().is_none());
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_unnamed_constructor_still_constructor() {
    let source = "class C { C() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(ctor) => {
            assert_eq!(ctor.name().name(), "C");
            assert!(ctor.constructor_name().is_none());
            assert!(!ctor.is_factory());
            assert!(!ctor.is_const());
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_named_constructor_still_constructor() {
    let source = "class C { C.named() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(ctor) => {
            assert_eq!(ctor.name().name(), "C");
            assert_eq!(ctor.constructor_name().map(|n| n.name()), Some("named"));
        }
        other => panic!("expected named constructor, got {other:?}"),
    }
}

#[test]
fn test_unnamed_and_named_constructor_with_initializers() {
    let source = "class C { int x; C(this.x); C.zero() : x = 0; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let c = only_class(prog);
    assert!(matches!(
        c.members().nth(1).unwrap(),
        ClassMember::Constructor(_)
    ));
    assert!(matches!(
        c.members().nth(2).unwrap(),
        ClassMember::Constructor(_)
    ));
}

#[test]
fn test_const_constructor_still_constructor() {
    let source = "class C { const C(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(ctor) => {
            assert_eq!(ctor.name().name(), "C");
            assert!(ctor.is_const());
        }
        other => panic!("expected const constructor, got {other:?}"),
    }
}

#[test]
fn test_factory_constructor_still_constructor() {
    let source = "class C { factory C.make() => C._(); C._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(ctor) => {
            assert!(ctor.is_factory());
            assert_eq!(ctor.constructor_name().map(|n| n.name()), Some("make"));
        }
        other => panic!("expected factory constructor, got {other:?}"),
    }
}

#[test]
fn test_generic_untyped_member_named_like_type_is_method() {
    // A constructor cannot declare type parameters, so `C<T>()` is a method.
    let source = "class C { C<T>() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "C");
            assert!(m.return_type().is_none());
            assert_eq!(m.type_params().len(), 1);
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn test_async_untyped_member_named_like_type_is_method() {
    // A constructor cannot be async, so `C() async {}` is a method.
    let source = "class C { C() async {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "C");
            assert_eq!(m.async_marker(), AsyncMarker::Async);
            assert!(m.return_type().is_none());
        }
        other => panic!("expected async method, got {other:?}"),
    }
}

#[test]
fn test_enum_untyped_member_is_method() {
    let source = "enum E { a, b; foo() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Enum(e) => match e.members().next().unwrap() {
            ClassMember::Method(m) => {
                assert_eq!(m.name().name(), "foo");
                assert!(m.return_type().is_none());
            }
            other => panic!("expected method in enum, got {other:?}"),
        },
        other => panic!("expected enum, got {other:?}"),
    }
}

#[test]
fn test_enum_constructor_is_constructor() {
    let source = "enum E { a(1), b(2); final int n; const E(this.n); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Enum(e) => {
            let ctor = e
                .members()
                .find_map(|m| match m {
                    ClassMember::Constructor(c) => Some(c),
                    _ => None,
                })
                .expect("expected a constructor member in the enum");
            assert_eq!(ctor.name().name(), "E");
            assert!(ctor.is_const());
        }
        other => panic!("expected enum, got {other:?}"),
    }
}

#[test]
fn test_mixin_untyped_member_is_method() {
    // Even a member named after the mixin is a method — mixins have no ctors.
    let source = "mixin M { M() {} foo() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Mixin(m) => {
            for member in m.members() {
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
    let source = "extension E on int { foo() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match prog.declarations().next().unwrap() {
        TopLevelDecl::Extension(e) => match e.members().next().unwrap() {
            ClassMember::Method(m) => {
                assert_eq!(m.name().name(), "foo");
                assert!(m.return_type().is_none());
            }
            other => panic!("expected method in extension, got {other:?}"),
        },
        other => panic!("expected extension, got {other:?}"),
    }
}

#[test]
fn test_extension_type_untyped_member_is_method() {
    let source = "extension type E(int it) { foo() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match prog.declarations().next().unwrap() {
        TopLevelDecl::ExtensionType(e) => match e.members().next().unwrap() {
            ClassMember::Method(m) => {
                assert_eq!(m.name().name(), "foo");
                assert!(m.return_type().is_none());
            }
            other => panic!("expected method in extension type, got {other:?}"),
        },
        other => panic!("expected extension type, got {other:?}"),
    }
}

#[test]
fn test_extension_type_named_constructor_is_constructor() {
    let source = "extension type E(int it) { E.zero() : it = 0; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match prog.declarations().next().unwrap() {
        TopLevelDecl::ExtensionType(e) => match e.members().next().unwrap() {
            ClassMember::Constructor(ctor) => {
                assert_eq!(ctor.name().name(), "E");
                assert_eq!(ctor.constructor_name().map(|n| n.name()), Some("zero"));
            }
            other => panic!("expected constructor in extension type, got {other:?}"),
        },
        other => panic!("expected extension type, got {other:?}"),
    }
}

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
    let source = "enum E { a.foo(1); const E.foo(this.n); final int n; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_eq!(errors.len(), 0, "errors: {errors:?}");
    let en = match prog.declarations().next().unwrap() {
        TopLevelDecl::Enum(e) => e,
        other => panic!("expected enum, got {other:?}"),
    };
    let ctor = en.variants().next().unwrap().constructor_name();
    assert_eq!(ctor.map(|i| i.name()), Some("foo"));
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

#[test]
fn static_record_return_type_with_generic_args() {
    // `static (A<int>, B<int>) m()` — a record return type carrying generic args,
    // reached through the modifier-prefixed member path.
    let source = "class C { static (A<int>, B<int>) m() => x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert!(m.is_static());
            assert_eq!(m.name().name(), "m");
            assert!(
                matches!(m.return_type(), Some(DartType::Record(_))),
                "return_type: {:?}",
                m.return_type()
            );
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn static_nullable_named_record_return_type() {
    // `static ({int a})? m()` — a nullable named-field record return type.
    let source = "class C { static ({int a})? m() => x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert!(m.is_static());
            assert!(matches!(m.return_type(), Some(DartType::Record(_))));
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn late_record_typed_field() {
    // `late (int, int) r;` — `late` must stay a modifier, not be read as the
    // member's own name when a record type follows.
    let source = "class C { late (int, int) r; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Field(f) => {
            assert!(f.is_late());
            assert!(matches!(f.field_type(), Some(DartType::Record(_))));
            assert_eq!(f.declarators().next().unwrap().name().name(), "r");
        }
        other => panic!("expected field, got {other:?}"),
    }
}

#[test]
fn annotation_before_nullable_record_return_type() {
    // `@override ()? m()` — the `()?` is the nullable empty-record return type, not
    // the annotation's argument list.
    let source = "class C { @override ()? m() => x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "m");
            assert!(
                m.annotations()
                    .any(|a| a.name().last().map(|i| i.name()) == Some("override")),
                "annotations: {:?}",
                m.annotations()
            );
            assert!(
                m.annotations().all(|a| a.args().is_none()),
                "annotation must carry no args"
            );
            assert!(matches!(m.return_type(), Some(DartType::Record(_))));
        }
        other => panic!("expected method, got {other:?}"),
    }
}

#[test]
fn builtin_override_identifier_as_field_name() {
    // `const override = 1;` — `override` is the field's own name here, not a
    // modifier to be silently consumed.
    let source = "class C { const override = 1; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Field(f) => {
            assert_eq!(f.keyword(), VarKeyword::Const);
            assert_eq!(f.declarators().next().unwrap().name().name(), "override");
        }
        other => panic!("expected field, got {other:?}"),
    }
}

// A member named after a modifier keyword can have an async/generator
// body. `external`/`late`/`covariant` before `()` followed by an `async`/`sync`
// body modifier is the member's OWN name (a method), not a leading modifier —
// the `record_type_prefixes_member` scan must treat `async`/`sync` after `)` as
// a body marker, not a member-name signal.

#[test]
fn test_member_named_external_with_async_body() {
    let source = "class C { external() async {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "external");
            assert!(!m.is_external(), "keyword is the name, not a modifier");
            assert_eq!(m.async_marker(), AsyncMarker::Async);
            assert!(m.return_type().is_none());
        }
        other => panic!("expected method `external`, got {other:?}"),
    }
}

#[test]
fn test_member_named_late_with_async_body() {
    let source = "class C { late() async {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "late");
            assert_eq!(m.async_marker(), AsyncMarker::Async);
            assert!(m.return_type().is_none());
        }
        other => panic!("expected method `late`, got {other:?}"),
    }
}

#[test]
fn test_member_named_covariant_with_sync_star_body() {
    let source = "class C { covariant() sync* {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "covariant");
            assert_eq!(m.async_marker(), AsyncMarker::SyncStar);
        }
        other => panic!("expected method `covariant`, got {other:?}"),
    }
}

#[test]
fn test_static_record_return_type_still_modifier() {
    // Control: a genuine record return type keeps `static` as a modifier.
    let source = "class C { static (int, int) m() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "m");
            assert!(m.is_static());
            assert!(matches!(m.return_type(), Some(DartType::Record(_))));
        }
        other => panic!("expected static method `m`, got {other:?}"),
    }
}

// A member named `async`/`sync` can be prefixed by a record
// return type. Here the modifier keyword (static/external/covariant/late) IS a
// modifier, the `(...)` IS the record return type, and `async`/`sync` is the
// member's own name — distinguished from a body modifier by the token that
// follows the name (`(`/`<`/`;`/`=`/`,` → name; `{`/`=>`/`*` → body marker).

#[test]
fn test_static_record_return_method_named_async() {
    let source = "class C { static (int, int) async() => (1, 2); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "async");
            assert!(m.is_static(), "`static` stays a modifier");
            assert!(
                !m.async_marker().is_async(),
                "trailing keyword is the name, not a body marker"
            );
            assert!(matches!(m.return_type(), Some(DartType::Record(_))));
        }
        other => panic!("expected static method `async`, got {other:?}"),
    }
}

#[test]
fn test_external_record_return_method_named_sync() {
    let source = "class C { external (int, int) sync(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Method(m) => {
            assert_eq!(m.name().name(), "sync");
            assert!(m.is_external(), "`external` stays a modifier");
            assert_eq!(m.async_marker(), AsyncMarker::Sync);
            assert!(matches!(m.return_type(), Some(DartType::Record(_))));
        }
        other => panic!("expected external method `sync`, got {other:?}"),
    }
}

#[test]
fn test_late_record_typed_field_named_async() {
    let source = "class C { late (int, int) async; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Field(f) => {
            assert_eq!(f.declarators().next().unwrap().name().name(), "async");
            assert!(f.is_late(), "`late` stays a modifier");
            assert!(matches!(f.field_type(), Some(DartType::Record(_))));
        }
        other => panic!("expected late field named `async`, got {other:?}"),
    }
}

#[test]
fn test_factory_is_field_name() {
    // `factory` followed by `=`/`;`/`,` is the field's own name, not the factory
    // keyword — the SDK accepts a field named `factory`.
    let source = "class C { late final factory = 1; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Field(f) => {
            assert_eq!(f.declarators().next().unwrap().name().name(), "factory");
            assert!(f.is_late());
            assert_eq!(f.keyword(), VarKeyword::Final);
        }
        other => panic!("expected field named `factory`, got {other:?}"),
    }
}

#[test]
fn test_factory_constructor_still_parses() {
    // Control: a genuine factory constructor (name follows `factory`) is intact.
    let source = "class C { factory C() => C._(); C._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    assert!(matches!(
        only_class(prog).members().next().unwrap(),
        ClassMember::Constructor(c) if c.is_factory()
    ));
}

#[test]
fn test_private_named_initializing_formal_accepted() {
    // `{required this._x}` is valid from language 3.12 (private named
    // parameters); the parser accepts it regardless of language version.
    let source = "class C { final int _x; C({required this._x}); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert_clean(&errors);
}

#[test]
fn test_private_named_super_formal_accepted() {
    let source = "class C extends B { C({super._x}); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert_clean(&errors);
}

#[test]
fn test_private_positional_initializing_formal_ok() {
    // Control: a *positional* private initializing formal is legal — the feature
    // only gates named private formals.
    let source = "class C { final int _x; C(this._x); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert_clean(&errors);
}

#[test]
fn test_public_named_initializing_formal_ok() {
    // Control: a public named initializing formal is always legal.
    let source = "class C { int x; C({required this.x}); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert_clean(&errors);
}

#[test]
fn test_missing_semicolon_after_getter_arrow_body() {
    // `num get a => 42` with no `;` before the next member must ERROR — falcon
    // used to silently recover the missing terminator.
    let source = "class A { num get a => 42 int get b => 42; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert!(!errors.is_empty(), "expected a missing-semicolon error");
    assert!(errors.iter().any(|e| e.message.contains("Semicolon")));
}

#[test]
fn test_getter_arrow_body_with_semicolon_ok() {
    // Control: the terminated form parses clean.
    let source = "class A { int get b => 42; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert_clean(&errors);
}

#[test]
fn test_const_factory_preserves_is_const() {
    let source = "class C { const factory C.i() = D; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(c) => {
            assert!(c.is_factory(), "expected a factory");
            assert!(c.is_const(), "the `const` modifier must be preserved");
            assert!(c.redirect().is_some(), "expected a redirecting factory");
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_non_const_factory_is_not_const() {
    // Control: a plain factory keeps `is_const: false`.
    let source = "class C { factory C.i() = D; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(c) => {
            assert!(c.is_factory());
            assert!(!c.is_const());
        }
        other => panic!("expected constructor, got {other:?}"),
    }
}

#[test]
fn test_constructor_call_block_is_initializer_lambda() {
    // Keep this odd recovery outcome pinned until a deliberate parser-behavior change.
    let source = "class C { var g; C(): g = (e) { f(); } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let ClassMember::Constructor(ctor) = only_class(prog).members().nth(1).unwrap() else {
        panic!("expected constructor: {prog:?}");
    };
    assert!(
        ctor.body().is_none(),
        "body belongs to the initializer lambda"
    );
    assert_eq!(ctor.initializers().len(), 1);
    let ConstructorInitializer::FieldInit(initializer) = ctor.initializers().next().unwrap() else {
        panic!("expected field initializer: {:?}", ctor.initializers());
    };
    let field = initializer.field();
    let value = initializer.value();
    assert_eq!(field.name(), "g");
    let Expr::FuncExpr(function) = value else {
        panic!("expected initializer lambda: {value:?}");
    };
    let params = function.params();
    let body = function.body();
    assert_eq!(function.async_marker(), AsyncMarker::Sync);
    assert_eq!(params.params().len(), 1);
    assert_eq!(params.params().next().unwrap().name().name(), "e");
    let FunctionBody::Block(body) = body else {
        panic!("expected lambda block: {body:?}");
    };
    assert_eq!(body.stmts().len(), 1);
    let Stmt::Expr(stmt) = body.stmts().next().unwrap() else {
        panic!("expected lambda call statement: {body:?}");
    };
    let Expr::Call(call) = stmt.expr() else {
        panic!("expected lambda call: {stmt:?}");
    };
    let callee = call.callee();
    let args = call.args();
    assert!(matches!(callee, Expr::Ident(id) if id.name() == "f"));
    assert!(args.positional().is_empty());
    assert!(args.named().is_empty());
}

#[test]
fn test_leading_async_method_splits_await_and_operand_statements() {
    // Only a trailing async marker reserves body keywords.
    let source = "class Foo { async void bar() { await x; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let ClassMember::Method(method) = only_class(prog).members().next().unwrap() else {
        panic!("expected method: {prog:?}");
    };
    assert_eq!(method.async_marker(), AsyncMarker::Async);
    let Some(FunctionBody::Block(body)) = method.body() else {
        panic!("expected block body: {:?}", method.body());
    };
    assert_eq!(body.stmts().len(), 2, "{body:?}");
    for (stmt, name, expr_span, stmt_span) in [
        (body.stmts().next().unwrap(), "await", (31, 36), (31, 36)),
        (body.stmts().nth(1).unwrap(), "x", (37, 38), (37, 39)),
    ] {
        let Stmt::Expr(stmt) = stmt else {
            panic!("expected identifier expression statement: {stmt:?}");
        };
        let Expr::Ident(id) = stmt.expr() else {
            panic!("expected identifier: {stmt:?}");
        };
        assert_eq!(id.name(), name);
        assert_eq!((id.span().start, id.span().end), expr_span);
        assert_eq!((stmt.span().start, stmt.span().end), stmt_span);
    }
}

#[test]
fn test_async_sync_star_method_has_async_star_ast_marker() {
    // The combined method marker differs from the trailing marker used in the body.
    let source = "class C { async m() sync* { yield 1; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let ClassMember::Method(method) = only_class(prog).members().next().unwrap() else {
        panic!("expected method: {prog:?}");
    };
    assert_eq!(method.async_marker(), AsyncMarker::AsyncStar);
    let Some(FunctionBody::Block(body)) = method.body() else {
        panic!("expected block body: {:?}", method.body());
    };
    assert_eq!(body.stmts().len(), 1);
    let Stmt::Yield(stmt) = body.stmts().next().unwrap() else {
        panic!("expected generator yield: {:?}", body.stmts());
    };
    assert!(!stmt.is_star());
    assert!(
        matches!(stmt.value(), Expr::IntLit(value) if value.value() == "1"),
        "{stmt:?}"
    );
}

#[test]
fn test_async_constructor_initializer_keeps_synchronous_body() {
    let source = "class C { var f; C(bool async) : f = async { var await = 1; await; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let ClassMember::Constructor(ctor) = only_class(prog).members().nth(1).unwrap() else {
        panic!("expected constructor: {prog:?}");
    };
    assert_eq!(ctor.initializers().len(), 1);
    let ConstructorInitializer::FieldInit(initializer) = ctor.initializers().next().unwrap() else {
        panic!("expected field initializer: {:?}", ctor.initializers());
    };
    let field = initializer.field();
    let value = initializer.value();
    assert_eq!(field.name(), "f");
    assert!(
        matches!(value, Expr::Ident(id) if id.name() == "async"),
        "{value:?}"
    );
    let Some(FunctionBody::Block(body)) = ctor.body() else {
        panic!("expected constructor body: {:?}", ctor.body());
    };
    assert_eq!(body.stmts().len(), 2, "{body:?}");
    let Stmt::LocalVar(declaration) = body.stmts().next().unwrap() else {
        panic!(
            "expected contextual identifier declaration: {:?}",
            body.stmts()
        );
    };
    assert_eq!(declaration.declarators().len(), 1);
    let declarator = declaration.declarators().next().unwrap();
    assert_eq!(declarator.name().name(), "await");
    assert_eq!(
        &source[declarator.name().span().start..declarator.name().span().end],
        "await"
    );
    assert!(
        matches!(declarator.initializer(), Some(Expr::IntLit(value)) if value.value() == "1"),
        "{declarator:?}"
    );
    let Stmt::Expr(statement) = body.stmts().nth(1).unwrap() else {
        panic!("expected contextual identifier use: {:?}", body.stmts());
    };
    let Expr::Ident(identifier) = statement.expr() else {
        panic!("expected await identifier: {statement:?}");
    };
    assert_eq!(identifier.name(), "await");
    assert_eq!(
        &source[identifier.span().start..identifier.span().end],
        "await"
    );
}

/// Dart 3.13 constructor syntax omits the type name: `new(`, `new name(`,
/// `factory(`, `factory name(`. Each maps onto the classic constructor shape,
/// with the enclosing type name spanning the keyword.
fn only_ctor(prog: Program<'_>) -> ConstructorDecl<'_> {
    match only_class(prog).members().next().unwrap() {
        ClassMember::Constructor(ctor) => ctor,
        other => panic!("expected constructor, got {other:?}"),
    }
}

fn ctor_name<'a>(ctor: ConstructorDecl<'a>) -> Option<&'a str> {
    ctor.constructor_name().map(|n| n.name())
}

#[test]
fn test_new_unnamed_constructor() {
    let source = "class C { new(this.x) : assert(x > 0); final int x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert!(!ctor.is_factory() && !ctor.is_const());
    assert_eq!(ctor.name().name(), "C");
    assert_eq!(
        &source[ctor.name().span().start..ctor.name().span().end],
        "new"
    );
    assert_eq!(ctor_name(ctor), None);
    assert_eq!(ctor.params().params().len(), 1);
    assert_eq!(ctor.initializers().len(), 1);
}

#[test]
fn test_const_new_constructor() {
    let source = "class C { const new() : super(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert!(ctor.is_const() && !ctor.is_factory());
    assert_eq!(ctor.name().name(), "C");
    assert!(ctor.body().is_none());
}

#[test]
fn test_new_named_constructor() {
    let source = "class C { new internal(int x) : this.other(x); C.other(int x); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert_eq!(ctor.name().name(), "C");
    assert_eq!(ctor_name(ctor), Some("internal"));
}

#[test]
fn test_factory_unnamed_constructor() {
    let source = "class C { factory(int x) => C._(); C._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert!(ctor.is_factory());
    assert_eq!(ctor.name().name(), "C");
    assert_eq!(ctor_name(ctor), None);
}

#[test]
fn test_factory_named_constructor() {
    let source = "class C { factory make(String m) => C._(); C._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert!(ctor.is_factory());
    assert_eq!(ctor.name().name(), "C");
    assert_eq!(ctor_name(ctor), Some("make"));
}

#[test]
fn test_const_redirecting_factory_named_constructor() {
    let source = "class C { const factory make() = C._; const C._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert!(ctor.is_factory() && ctor.is_const() && ctor.redirect().is_some());
    assert_eq!(ctor_name(ctor), Some("make"));
}

#[test]
fn test_classic_factory_keeps_type_name() {
    let source = "class C { factory C(int x) => C._(); C._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert_clean(&errors);
    let ctor = only_ctor(tree.program(source));
    assert_eq!(ctor.name().name(), "C");
    assert_eq!(ctor_name(ctor), None);
}

#[test]
fn test_factory_in_mixin_is_method() {
    let source = "mixin M { factory() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    assert!(
        matches!(prog.declarations().next().unwrap(), TopLevelDecl::Mixin(m) if matches!(m.members().next().unwrap(), ClassMember::Method(_)))
    );
}

#[test]
fn test_semicolon_empty_class_body() {
    let source = "abstract class Marker;\nmixin M;\nclass After {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    assert!(only_class_named(prog, "Marker").members().is_empty());
    assert_eq!(prog.declarations().len(), 3);
}

fn only_class_named<'a>(prog: Program<'a>, name: &str) -> ClassDecl<'a> {
    prog.declarations()
        .find_map(|decl| match decl {
            TopLevelDecl::Class(class) if class.name().name() == name => Some(class),
            _ => None,
        })
        .expect("class")
}

#[test]
fn test_native_member_bodies() {
    let source = "class C { int get length native; void f() native \"g\"; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    let ClassMember::Getter(getter) = only_class(prog).members().next().unwrap() else {
        panic!("expected getter: {prog:?}");
    };
    assert!(matches!(getter.body(), Some(FunctionBody::Native(body)) if body.literal().is_none()));
}

#[test]
fn test_setter_parameter_metadata_and_function_type() {
    let source =
        "class C { set length(@nullCheck covariant int v) {} set onPause(void handler()?) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let prog = tree.program(source);
    assert_clean(&errors);
    assert_eq!(only_class(prog).members().len(), 2);
    let ClassMember::Setter(setter) = only_class(prog).members().next().unwrap() else {
        panic!("expected setter: {prog:?}");
    };
    assert!(setter.is_covariant());
    assert_eq!(setter.param_annotations().len(), 1);
    assert_eq!(
        setter
            .param_annotations()
            .next()
            .unwrap()
            .name()
            .next()
            .unwrap()
            .name(),
        "nullCheck"
    );
    let ClassMember::Setter(setter) = only_class(prog).members().nth(1).unwrap() else {
        panic!("expected function-typed setter");
    };
    assert!(!setter.is_covariant());
    let Some(DartType::Function(function)) = setter.param_type() else {
        panic!("expected function type, got {:?}", setter.param_type());
    };
    assert!(function.is_nullable());
    assert!(function.params().is_empty());
    assert!(matches!(function.return_type(), Some(DartType::Void(_))));
}

#[test]
fn test_extension_type_representation_trailing_comma() {
    let source = "extension type E(\n  p.T _value,\n) implements Object {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let _prog = tree.program(source);
    assert_clean(&errors);
}
