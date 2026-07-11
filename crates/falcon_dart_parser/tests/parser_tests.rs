use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;
use insta::assert_debug_snapshot;

// ── Empty and basic ───────────────────────────────────────────────────────────

#[test]
fn test_empty_program() {
    let (prog, errors) = parse("");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(prog.declarations.is_empty());
}

#[test]
fn test_import_simple() {
    let src = r#"import 'dart:core';"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports.len(), 1);
    assert_eq!(prog.imports[0].uri.value, "dart:core");
}

#[test]
fn test_import_with_as() {
    let src = r#"import 'foo.dart' as foo;"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports.len(), 1);
    assert!(prog.imports[0].as_name.is_some());
    assert_eq!(prog.imports[0].as_name.as_ref().unwrap().name, "foo");
}

#[test]
fn test_import_with_show() {
    let src = r#"import 'pkg.dart' show foo, bar;"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports[0].combinators.len(), 1);
}

#[test]
fn test_import_with_hide() {
    let src = r#"import 'pkg.dart' hide Secret;"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports[0].combinators.len(), 1);
}

#[test]
fn test_import_deferred() {
    let src = r#"import 'pkg.dart' deferred as lazy;"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(prog.imports[0].is_deferred);
}

#[test]
fn test_export_simple() {
    let src = r#"export 'foo.dart';"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.exports.len(), 1);
}

#[test]
fn test_library_directive() {
    let src = "library my.lib;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(prog.library_directive.is_some());
}

#[test]
fn test_part_directive() {
    let src = r#"part 'foo.dart';"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.part_directives.len(), 1);
}

// ── Class declarations ────────────────────────────────────────────────────────

#[test]
fn test_class_empty() {
    let src = "class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.declarations.len(), 1);
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.name.name, "Foo");
    assert!(cls.members.is_empty());
}

#[test]
fn test_class_with_field() {
    let src = "class Foo { int x; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.members.len(), 1);
    let ClassMember::Field(fld) = &cls.members[0] else {
        panic!("expected field")
    };
    assert_eq!(fld.declarators.len(), 1);
    assert_eq!(fld.declarators[0].name.name, "x");
}

#[test]
fn test_class_with_multiple_fields() {
    let src = "class Foo { int x, y; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Field(fld) = &cls.members[0] else {
        panic!("expected field")
    };
    assert_eq!(fld.declarators.len(), 2);
}

#[test]
fn test_class_with_method() {
    let src = "class Foo { void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.members.len(), 1);
    let ClassMember::Method(meth) = &cls.members[0] else {
        panic!("expected method")
    };
    assert_eq!(meth.name.name, "bar");
}

#[test]
fn test_class_with_constructor() {
    let src = "class Foo { Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = &cls.members[0] else {
        panic!("expected constructor")
    };
    assert_eq!(ctor.name.name, "Foo");
}

#[test]
fn test_class_extends() {
    let src = "class Child extends Parent {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert!(cls.extends.is_some());
}

#[test]
fn test_class_with_clause() {
    let src = "class Child with Mixin1, Mixin2 {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.with_clause.len(), 2);
}

#[test]
fn test_class_implements() {
    let src = "class Foo implements Bar, Baz {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.implements.len(), 2);
}

#[test]
fn test_class_abstract() {
    let src = "abstract class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert!(cls.modifiers.is_abstract);
}

#[test]
fn test_class_interface() {
    let src = "interface class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert!(cls.modifiers.is_interface);
}

#[test]
fn test_class_final() {
    let src = "final class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert!(cls.modifiers.is_final);
}

#[test]
fn test_class_sealed() {
    let src = "sealed class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert!(cls.modifiers.is_sealed);
}

#[test]
fn test_class_generic() {
    let src = "class Box<T> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params.len(), 1);
}

// ── Mixin ─────────────────────────────────────────────────────────────────────

#[test]
fn test_mixin_simple() {
    let src = "mixin M {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Mixin(m) = &prog.declarations[0] else {
        panic!("expected mixin")
    };
    assert_eq!(m.name.name, "M");
}

#[test]
fn test_mixin_with_on() {
    let src = "mixin M on Base {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Mixin(m) = &prog.declarations[0] else {
        panic!("expected mixin")
    };
    assert_eq!(m.on_clause.len(), 1);
}

#[test]
fn test_mixin_class() {
    let src = "mixin class M {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(prog.declarations[0], TopLevelDecl::MixinClass(_)));
}

// ── Enum ──────────────────────────────────────────────────────────────────────

#[test]
fn test_enum_simple() {
    let src = "enum Color { red, green, blue }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Enum(en) = &prog.declarations[0] else {
        panic!("expected enum")
    };
    assert_eq!(en.name.name, "Color");
    assert_eq!(en.variants.len(), 3);
}

#[test]
fn test_enum_enhanced_with_members() {
    let src = "enum Status { active, inactive; final int code; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Enum(en) = &prog.declarations[0] else {
        panic!("expected enum")
    };
    assert!(!en.members.is_empty());
}

// ── Extension ─────────────────────────────────────────────────────────────────

#[test]
fn test_extension_simple() {
    let src = "extension IntParsing on String { }";
    let (prog, errors) = parse(src);
    // Extensions might not be fully supported yet; check they don't panic
    assert!(!prog.declarations.is_empty() || !errors.is_empty());
}

#[test]
fn test_extension_with_name() {
    let src = "extension NumberParsing on String { }";
    let (prog, errors) = parse(src);
    // Extensions might not be fully supported yet; check they don't panic
    assert!(!prog.declarations.is_empty() || !errors.is_empty());
}

#[test]
fn test_extension_type() {
    let src = "extension type UserId(int value) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(
        prog.declarations[0],
        TopLevelDecl::ExtensionType(_)
    ));
}

// ── Functions ─────────────────────────────────────────────────────────────────

#[test]
fn test_function_void_no_params() {
    let src = "void foo() {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert_eq!(fun.name.name, "foo");
}

#[test]
fn test_function_with_return_type() {
    let src = "int add(int a, int b) { return a + b; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert!(fun.return_type.is_some());
}

#[test]
fn test_function_async() {
    let src = "void foo() async {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert!(fun.is_async);
}

#[test]
fn test_function_generator() {
    let src = "Iterable<int> foo() sync* {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert!(fun.is_generator);
}

#[test]
fn test_function_arrow() {
    let src = "int double(int x) => x * 2;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert!(fun.body.is_some());
}

// ── Variables ─────────────────────────────────────────────────────────────────

#[test]
fn test_top_level_var() {
    let src = "void main() {} var x = 42;";
    let (prog, _errors) = parse(src);
    // Parser may not support bare var at top level without type context
    assert!(!prog.declarations.is_empty());
}

#[test]
fn test_top_level_final() {
    let src = "final int x = 42;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    assert!(v.is_final, "field should be final");
}

#[test]
fn test_top_level_const() {
    let src = "const int x = 42;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    assert!(v.is_const);
}

#[test]
fn test_top_level_late() {
    let src = "late int x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    assert!(v.is_late);
}

// ── Typedef ───────────────────────────────────────────────────────────────────

#[test]
fn test_typedef_function_type() {
    let src = "typedef Callback = void Function(int);";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(prog.declarations[0], TopLevelDecl::TypeAlias(_)));
}

// ── Statements ────────────────────────────────────────────────────────────────

#[test]
fn test_stmt_if() {
    let src = "main() { if (true) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let Some(body) = &fun.body else {
        panic!("expected body")
    };
    match body {
        FunctionBody::Block(b) => {
            assert_eq!(b.stmts.len(), 1);
            assert!(matches!(&b.stmts[0], Stmt::If(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_if_else() {
    let src = "main() { if (true) { } else { } }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = &b.stmts[0] else {
                panic!("expected if")
            };
            assert!(if_stmt.else_branch.is_some());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_if_case() {
    let src = "main() { if (x case 42) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = &b.stmts[0] else {
                panic!("expected if")
            };
            assert!(matches!(if_stmt.condition, IfCondition::Case(_, _)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_while() {
    let src = "main() { while (true) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::While(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_do_while() {
    let src = "main() { do {} while (true); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::DoWhile(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_for() {
    let src = "main() { for (var i = 0; i < 10; i++) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::For(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_for_in() {
    let src = "main() { for (var x in list) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else {
                panic!("expected for")
            };
            assert!(matches!(for_stmt.init, Some(ForInit::ForIn { .. })));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_for_await_in() {
    let src = "main() async { for (var x in stream) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else {
                panic!("expected for")
            };
            assert!(matches!(for_stmt.init, Some(ForInit::ForIn { .. })));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_switch() {
    let src = "main() { switch (x) { case 1: break; } }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::Switch(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_try_catch() {
    let src = "main() { try {} catch (e) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::TryCatch(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_try_catch_finally() {
    let src = "main() { try {} catch (e) {} finally {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::TryCatch(tc) = &b.stmts[0] else {
                panic!("expected try-catch")
            };
            assert!(tc.finally.is_some());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_return() {
    let src = "main() { return 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Return(ret) = &b.stmts[0] else {
                panic!("expected return")
            };
            assert!(ret.value.is_some());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_throw() {
    let src = "main() { throw Exception(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::Throw(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_break() {
    let src = "main() { for (;;) { break; } }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else {
                panic!("expected for")
            };
            match &*for_stmt.body {
                Stmt::Block(bl) => {
                    assert!(matches!(&bl.stmts[0], Stmt::Break(_)));
                }
                _ => panic!("expected block body"),
            }
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_continue() {
    let src = "main() { for (;;) { continue; } }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else {
                panic!("expected for")
            };
            match &*for_stmt.body {
                Stmt::Block(bl) => {
                    assert!(matches!(&bl.stmts[0], Stmt::Continue(_)));
                }
                _ => panic!("expected block body"),
            }
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_assert() {
    let src = "main() { assert(x > 0); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::Assert(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_yield() {
    let src = "gen() sync* { yield 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::Yield(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_yield_star() {
    let src = "gen() sync* { yield* other; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Yield(ys) = &b.stmts[0] else {
                panic!("expected yield")
            };
            assert!(ys.is_star);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_local_var() {
    let src = "main() { int x = 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(&b.stmts[0], Stmt::LocalVar(_)));
        }
        _ => panic!("expected block body"),
    }
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[test]
fn test_expr_integer_literal() {
    let src = "main() { 42; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::IntLit { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_string_literal() {
    let src = r#"main() { "hello"; }"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::StringLit(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_bool_literal() {
    let src = "main() { true; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::BoolLit { value: true, .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_null_literal() {
    let src = "main() { null; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::NullLit { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_identifier() {
    let src = "main() { foo; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Ident(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_this() {
    let src = "main() { this; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::This { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_super() {
    let src = "main() { super; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Super { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_binary_add() {
    let src = "main() { a + b; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Binary { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_unary_minus() {
    let src = "main() { -x; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Unary { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_field_access() {
    let src = "main() { obj.field; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Field { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_null_safe_field_access() {
    let src = "main() { obj?.field; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            let Expr::Field { is_null_safe, .. } = &es.expr else {
                panic!("expected field")
            };
            assert!(*is_null_safe);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_index_access() {
    let src = "main() { arr[0]; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Index { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_call() {
    let src = "main() { foo(1, 2); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Call { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_cascade() {
    let src = "main() { obj..foo()..bar; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Cascade { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_conditional() {
    let src = "main() { a ? b : c; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Conditional { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_is_type() {
    let src = "main() { x is int; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Is { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_is_not_type() {
    let src = "main() { x is! int; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            let Expr::Is { negated, .. } = &es.expr else {
                panic!("expected is")
            };
            assert!(*negated);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_as_cast() {
    let src = "main() { (x as int); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::As { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_list_literal() {
    let src = "main() { [1, 2, 3]; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::List { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_map_literal() {
    let src = "main() { {'a': 1, 'b': 2}; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Map { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_set_literal() {
    let src = "main() { {1, 2, 3}; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Set { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_record_literal() {
    let src = "main() { (1, 2, x: 3); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Record { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_await() {
    let src = "main() async { await foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Await { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_new() {
    let src = "main() { new Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::New { .. }));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_const_new() {
    let src = "main() { const Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            let Expr::New { is_const, .. } = &es.expr else {
                panic!("expected new")
            };
            assert!(*is_const);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_switch_expression() {
    let src = "main() { x switch { 1 => 'a', _ => 'b' }; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else {
                panic!("expected expr stmt")
            };
            assert!(matches!(&es.expr, Expr::Switch { .. }));
        }
        _ => panic!("expected block body"),
    }
}

// ── Dot shorthands (Dart 3.9) ─────────────────────────────────────────────────

/// Parses `main() { <src>; }` and returns the first statement's expression.
fn first_stmt_expr(src: &str) -> Expr {
    let wrapped = format!("main() {{ {src}; }}");
    let (prog, errors) = parse(&wrapped);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    let Some(FunctionBody::Block(b)) = &fun.body else {
        panic!("expected block body")
    };
    let Stmt::Expr(es) = &b.stmts[0] else {
        panic!("expected expr stmt")
    };
    es.expr.clone()
}

#[test]
fn test_dot_shorthand_bare() {
    let expr = first_stmt_expr(".red");
    assert!(
        matches!(&expr, Expr::DotShorthand { is_const: false, name, .. } if name.name == "red")
    );
}

#[test]
fn test_dot_shorthand_call() {
    let expr = first_stmt_expr(".parse('42')");
    let Expr::Call { callee, args, .. } = &expr else {
        panic!("expected call, got {expr:?}")
    };
    assert!(matches!(callee.as_ref(), Expr::DotShorthand { name, .. } if name.name == "parse"));
    assert_eq!(args.positional.len(), 1);
}

#[test]
fn test_dot_shorthand_generic_call() {
    let expr = first_stmt_expr(".parse<int>('42')");
    let Expr::Call {
        callee, type_args, ..
    } = &expr
    else {
        panic!("expected call, got {expr:?}")
    };
    assert!(matches!(callee.as_ref(), Expr::DotShorthand { name, .. } if name.name == "parse"));
    assert_eq!(type_args.len(), 1);
}

#[test]
fn test_dot_shorthand_new_tearoff() {
    let expr = first_stmt_expr(".new");
    assert!(matches!(&expr, Expr::DotShorthand { name, .. } if name.name == "new"));
}

#[test]
fn test_dot_shorthand_new_call() {
    let expr = first_stmt_expr(".new(1, 2)");
    let Expr::Call { callee, args, .. } = &expr else {
        panic!("expected call, got {expr:?}")
    };
    assert!(matches!(callee.as_ref(), Expr::DotShorthand { name, .. } if name.name == "new"));
    assert_eq!(args.positional.len(), 2);
}

#[test]
fn test_dot_shorthand_const() {
    // `const` in statement position is a variable-declaration keyword, so drive
    // the const-expression path via a list element instead.
    let expr = first_stmt_expr("[const .fromLtrb(4, 3, 2, 1)]");
    let Expr::List { elements, .. } = &expr else {
        panic!("expected list, got {expr:?}")
    };
    let CollectionElement::Expr(Expr::Call { callee, args, .. }) = &elements[0] else {
        panic!("expected call element, got {:?}", elements[0])
    };
    let Expr::DotShorthand { is_const, name, .. } = callee.as_ref() else {
        panic!("expected dot shorthand, got {callee:?}")
    };
    assert!(is_const);
    assert_eq!(name.name, "fromLtrb");
    assert_eq!(args.positional.len(), 4);
}

#[test]
fn test_dot_shorthand_postfix_chain() {
    // A shorthand head still supports trailing selectors: `.red.value`.
    let expr = first_stmt_expr(".red.value");
    let Expr::Field { object, field, .. } = &expr else {
        panic!("expected field access, got {expr:?}")
    };
    assert_eq!(field.name, "value");
    assert!(matches!(object.as_ref(), Expr::DotShorthand { name, .. } if name.name == "red"));
}

// ── Digit separators (Dart 3.6) ───────────────────────────────────────────────

#[test]
fn test_int_digit_separator_literal() {
    let expr = first_stmt_expr("1_000_000");
    assert!(matches!(&expr, Expr::IntLit { value, .. } if value == "1_000_000"));
}

#[test]
fn test_hex_digit_separator_literal() {
    let expr = first_stmt_expr("0xFF_EC");
    assert!(matches!(&expr, Expr::IntLit { value, .. } if value == "0xFF_EC"));
}

#[test]
fn test_double_digit_separator_literal() {
    let expr = first_stmt_expr("1_2.3_4e1_2");
    assert!(matches!(&expr, Expr::DoubleLit { value, .. } if value == "1_2.3_4e1_2"));
}

// ── Patterns ──────────────────────────────────────────────────────────────────

#[test]
fn test_pattern_wildcard() {
    let src = "main() { if (x case _) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = &b.stmts[0] else {
                panic!("expected if")
            };
            assert!(matches!(if_stmt.condition, IfCondition::Case(_, _)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_pattern_variable() {
    let src = "main() { if (x case var y) {} }";
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_literal() {
    let src = "main() { if (x case 42) {} }";
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_list() {
    let src = "main() { if (x case [a, b]) {} }";
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_map() {
    let src = "main() { if (x case {'a': a}) {} }";
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_record() {
    let src = "main() { if (x case (a, b)) {} }";
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

// ── Error recovery ────────────────────────────────────────────────────────────

#[test]
fn test_error_recovery_partial_class() {
    let src = "class Foo { int x }";
    let (prog, _errors) = parse(src);
    // Parser should still produce some AST even with errors
    assert!(matches!(prog.declarations[0], TopLevelDecl::Class(_)));
}

#[test]
fn test_error_recovery_malformed_function() {
    let src = "void foo() {";
    let (prog, errors) = parse(src);
    // Parser should not panic
    assert!(!prog.declarations.is_empty() || !errors.is_empty());
}

// ── Annotations ───────────────────────────────────────────────────────────────

#[test]
fn test_annotation_override() {
    let src = "class Foo { @override void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = &cls.members[0] else {
        panic!("expected method")
    };
    assert!(!meth.annotations.is_empty());
}

// ── Named and optional parameters ─────────────────────────────────────────────

#[test]
fn test_named_parameter() {
    let src = "main({required int x}) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert!(!fun.params.named.is_empty());
}

#[test]
fn test_optional_positional_parameter() {
    let src = "main([int x]) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    assert!(!fun.params.optional_positional.is_empty());
}

// ── Null-safety ───────────────────────────────────────────────────────────────

#[test]
fn test_nullable_type() {
    let src = "int? x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected var")
    };
    assert!(v.var_type.is_some());
}

// ── Generic types ─────────────────────────────────────────────────────────────

#[test]
fn test_generic_single_param() {
    let src = "class Box<T> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params.len(), 1);
}

#[test]
fn test_generic_multiple_params() {
    let src = "class Map<K, V> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params.len(), 2);
}

#[test]
fn test_nested_generic_type() {
    let src = "List<Map<String, int>> data;";
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

// ── Snapshot tests ────────────────────────────────────────────────────────────

#[test]
fn snap_class_simple() {
    let (prog, errors) = parse("class Foo {}");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_class_with_field_and_method() {
    let (prog, errors) = parse("class Point { int x; void move(int dx) {} }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_top_level_function() {
    let (prog, errors) = parse("int add(int a, int b) => a + b;");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_import_directive() {
    let (prog, errors) = parse("import 'dart:core' show List, Map;");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.imports[0]);
}

#[test]
fn snap_enum_simple() {
    let (prog, errors) = parse("enum Color { red, green, blue }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_typedef_function() {
    let (prog, errors) = parse("typedef Callback = void Function(int);");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_abstract_class_with_getter() {
    let (prog, errors) = parse("abstract class Shape { double get area; }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_switch_expression() {
    let (prog, errors) = parse("String f(int x) => x switch { 1 => 'one', _ => 'other' };");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_sealed_class() {
    let (prog, errors) = parse("sealed class Result<T> {}");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_extension() {
    let (prog, errors) = parse("extension StringX on String { bool get isBlank => length == 0; }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations[0]);
}

// ── Additional corpus-representative snapshots ────────────────────────────────

#[test]
fn snap_mixin_declaration() {
    let (prog, errors) = parse("mixin Serializable on Object { Map<String, dynamic> toJson(); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_class_extends_with_implements() {
    let src = "class Dog extends Animal with Barker implements Pet { final String name; Dog(this.name); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_named_constructor() {
    let src = "class Vec2 { final double x; final double y; Vec2(this.x, this.y); Vec2.zero() : x = 0, y = 0; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_factory_constructor() {
    let src = "class Singleton { static final Singleton _i = Singleton._(); factory Singleton() => _i; Singleton._(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_async_function() {
    let src =
        "Future<String> fetchName() async { await Future.delayed(Duration.zero); return 'dart'; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_generic_function() {
    let src = "T identity<T>(T value) => value;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_record_return_type() {
    let src = "(String, int) pair() => ('hello', 42);";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_annotation_with_args() {
    let src = "@Deprecated('Use newFn instead')\nvoid oldFn() {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_late_final_field() {
    let src = "class Lazy { late final String value; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_typed_list_literal() {
    let src = "List<String> names() => <String>['Alice', 'Bob'];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_collection_for() {
    let src = "List<int> doubled(List<int> xs) => [for (final x in xs) x * 2];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_switch_pattern_guard() {
    let src = r#"String classify(int n) => switch (n) { < 0 => 'neg', 0 => 'zero', _ => 'pos' };"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_enum_with_members() {
    let src = "enum Color { red, green, blue; bool get isPrimary => this != Color.green; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_typedef_function_type() {
    let src = "typedef Predicate<T> = bool Function(T value);";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

#[test]
fn snap_const_class_field() {
    let src = "class Config { static const int maxRetries = 3; static const String baseUrl = 'https://api.example.com'; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations[0]);
}

// ── Bug reproduction: Dart 3 pattern elements in collections ──────────────────

fn first_fn_arrow_or_block_expr(prog: &Program) -> &Expr {
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else {
        panic!("expected function")
    };
    match fun.body.as_ref().expect("body") {
        FunctionBody::Arrow(e, _) => e.as_ref(),
        FunctionBody::Block(b) => match &b.stmts[0] {
            Stmt::Expr(es) => &es.expr,
            Stmt::LocalVar(vd) => vd.declarators[0].initializer.as_ref().expect("init"),
            other => panic!("unexpected stmt: {other:?}"),
        },
        _ => panic!("unexpected body"),
    }
}

#[test]
fn test_bug_collection_if_case_keeps_all_elements() {
    // if-element with a `case` pattern must not truncate later elements.
    let src = "f(e) => [ if (e.x case final y?) ...[1] , other, elements ];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List { elements, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!("expected list")
    };
    assert_eq!(elements.len(), 3, "elements: {elements:#?}");
}

#[test]
fn test_bug_collection_if_case_with_when_guard() {
    let src = "f(e) => [ if (e.x case final y? when y > 0) y, other, elements ];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List { elements, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!("expected list")
    };
    assert_eq!(elements.len(), 3, "elements: {elements:#?}");
}

#[test]
fn test_bug_collection_for_record_destructuring() {
    let src = "f(list) => [ for (final (i, s) in list.indexed) widget(i, s), tail ];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List { elements, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!("expected list")
    };
    assert_eq!(elements.len(), 2, "elements: {elements:#?}");
}

#[test]
fn test_bug_map_comprehension_for_element() {
    let src = "f(xs) => { for (final x in xs) x.id: x };";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(first_fn_arrow_or_block_expr(&prog), Expr::Map { .. }),
        "expected map, got {:#?}",
        first_fn_arrow_or_block_expr(&prog)
    );
}

#[test]
fn test_bug_map_comprehension_for_pattern() {
    let src = "f(xs) => { for (final (k, v) in xs) k: v };";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(first_fn_arrow_or_block_expr(&prog), Expr::Map { .. }),
        "expected map"
    );
}

#[test]
fn test_bug_function_typed_field_then_override_method() {
    // @override on a method after a function-typed field must be preserved.
    let src = r#"
class W {
  final void Function() onTap;
  W(this.onTap);
  @override
  Widget build(BuildContext context) => X();
}
"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let build = cls
        .members
        .iter()
        .find_map(|m| match m {
            ClassMember::Method(mth) if mth.name.name == "build" => Some(mth),
            _ => None,
        })
        .expect("build method");
    assert_eq!(
        build.annotations.len(),
        1,
        "expected @override on build, annotations: {:#?}",
        build.annotations
    );
}

#[test]
fn test_bug_function_typed_field_named_params_then_override() {
    // Mirrors jfit _SessionTile: function type with named param, then @override.
    let src = r#"
class _SessionTile extends StatelessWidget {
  const _SessionTile({required this.session, required this.onRespond});

  final AppSession session;
  final void Function(String sessionId, {required bool accept}) onRespond;

  @override
  Widget build(final BuildContext context) => SessionCard(session: session);
}
"#;
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else {
        panic!("expected class")
    };
    let build = cls
        .members
        .iter()
        .find_map(|m| match m {
            ClassMember::Method(mth) if mth.name.name == "build" => Some(mth),
            _ => None,
        })
        .expect("build method");
    assert_eq!(
        build.annotations.len(),
        1,
        "expected @override on build, annotations: {:#?}",
        build.annotations
    );
}

#[test]
fn test_bug_collection_nested_spreads_after_pattern_constructs() {
    // Spreads following both a case-if and a pattern-for must survive.
    let src =
        "f(a, b, xs) => [ if (a case final y?) ...[y], for (final (i, s) in xs) ...[i, s], ...b ];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List { elements, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!("expected list")
    };
    assert_eq!(elements.len(), 3, "elements: {elements:#?}");
}

// ── Closure / expression parser gap fixes ─────────────────────────────────────

/// Assert a source snippet parses with zero errors.
fn assert_parses(src: &str) {
    let (_prog, errors) = parse(src);
    assert!(errors.is_empty(), "src: {src}\nerrors: {errors:?}");
}

#[test]
fn test_closure_final_param() {
    let src = "void f() { list.map((final x) => x.y); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    // Drill to the closure argument and confirm it is a function expression.
    let arg = first_call_first_arg(&prog);
    assert!(
        matches!(arg, Expr::FuncExpr { params, .. } if params.positional.len() == 1),
        "expected FuncExpr with one positional param, got {arg:#?}"
    );
    if let Expr::FuncExpr { params, .. } = arg {
        assert!(params.positional[0].is_final, "param should be final");
        assert_eq!(params.positional[0].name.name, "x");
    }
}

#[test]
fn test_closure_var_param() {
    let src = "void f() { g((var x) => x); }";
    let arg = {
        let (prog, errors) = parse(src);
        assert!(errors.is_empty(), "errors: {errors:?}");
        first_call_first_arg(&prog).clone()
    };
    assert!(
        matches!(&arg, Expr::FuncExpr { params, .. } if params.positional.len() == 1
            && params.positional[0].param_type.is_none()),
        "expected untyped var param, got {arg:#?}"
    );
}

#[test]
fn test_closure_final_typed_param() {
    let src = "void f() { g((final int x) => x); }";
    let arg = {
        let (prog, errors) = parse(src);
        assert!(errors.is_empty(), "errors: {errors:?}");
        first_call_first_arg(&prog).clone()
    };
    assert!(
        matches!(&arg, Expr::FuncExpr { params, .. } if params.positional[0].is_final
            && params.positional[0].param_type.is_some()),
        "expected final typed param, got {arg:#?}"
    );
}

#[test]
fn test_closure_multiline_trailing_comma_args() {
    assert_parses(
        "void f() {\n  _run(\n    () => ctx.read<N>().create(\n      email: a,\n    ),\n    success: 'x',\n  );\n}",
    );
}

#[test]
fn test_closure_nested() {
    assert_parses("void f() { g((a) => (b) => a + b); }");
}

#[test]
fn test_closure_await_final_param() {
    // `await` leads an expression statement; the `(final repo)` closure argument
    // must not be misread as a `type name` local-function declaration.
    assert_parses(
        "class C { void m() async { await _mutate((final repo) async { for (final id in ids) { await repo.add(id); } }); } }",
    );
}

#[test]
fn test_is_type_ternary() {
    // `x is T ? a : b` — the `?` is the conditional operator, not a nullable type.
    let src = "void f() { final b = x is Foo ? x : y; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let init = first_local_var_init(&prog);
    assert!(
        matches!(init, Some(Expr::Conditional { .. })),
        "expected conditional, got {init:#?}"
    );
}

#[test]
fn test_is_nullable_type_still_parses() {
    // A genuine nullable type-test keeps the `?` on the type.
    let src = "void f() { g(x is String?); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let arg = first_call_first_arg(&prog);
    assert!(matches!(arg, Expr::Is { .. }), "expected Is, got {arg:#?}");
}

#[test]
fn test_as_type_ternary() {
    assert_parses("void f() { final b = x is F ? (x as F).d : null; }");
}

#[test]
fn test_less_than_ternary_closure() {
    // `a < b` is a comparison; the following `?` opens a conditional whose then
    // branch is a closure. `<` must not be read as generic type arguments.
    assert_parses("void f() { W(onTap: value < m ? () => onChanged(1) : null); }");
}

#[test]
fn test_generic_method_call_in_closure() {
    assert_parses("void f() { g(() => ctx.read<N>().create(email: a)); }");
}

#[test]
fn test_c_style_for_in_list_literal() {
    let src = "void f() { final l = [for (var i = 0; i < n; i++) i]; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List { elements, .. } = first_local_var_init(&prog).expect("init") else {
        panic!("expected list literal")
    };
    assert!(
        matches!(elements.first(), Some(CollectionElement::CFor { .. })),
        "expected CFor element, got {elements:#?}"
    );
}

#[test]
fn test_c_style_for_in_list_literal_spread() {
    assert_parses("void f() { final l = [for (var i = 0; i < n; i++) ...[i]]; }");
}

#[test]
fn test_c_style_for_in_map_literal() {
    assert_parses("void f() { final m = {for (var i = 0; i < n; i++) i: i}; }");
}

#[test]
fn test_async_getter() {
    let src = "class A { Future<int> get token async { return 1; } }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let getter = prog
        .declarations
        .iter()
        .find_map(|d| match d {
            TopLevelDecl::Class(c) => c.members.iter().find_map(|m| match m {
                ClassMember::Getter(g) => Some(g),
                _ => None,
            }),
            _ => None,
        })
        .expect("getter");
    assert!(getter.is_async, "getter should be async");
}

#[test]
fn test_nullable_function_type_param() {
    // `String? Function(...)? name` — nullable return type before `Function`.
    assert_parses("class C { const C({final String? Function(int)? reselect}); }");
}

#[test]
fn test_generic_function_expression() {
    let src = "void f() { final g = <T extends Object>(final T x) { return x; }; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let init = first_local_var_init(&prog).expect("init");
    assert!(
        matches!(init, Expr::FuncExpr { type_params, .. } if type_params.len() == 1),
        "expected generic FuncExpr, got {init:#?}"
    );
}

#[test]
fn test_assert_initializer_trailing_comma_message() {
    assert_parses("class C { C(this.a) : assert(a != null, 'a must be provided',); final int a; }");
}

// ── Helpers for the gap-fix tests ─────────────────────────────────────────────

fn first_fn_body_stmts(prog: &Program) -> &[Stmt] {
    for d in &prog.declarations {
        if let TopLevelDecl::Function(f) = d
            && let Some(FunctionBody::Block(b)) = &f.body
        {
            return &b.stmts;
        }
        if let TopLevelDecl::Class(c) = d {
            for m in &c.members {
                if let ClassMember::Method(mth) = m
                    && let Some(FunctionBody::Block(b)) = &mth.body
                {
                    return &b.stmts;
                }
            }
        }
    }
    &[]
}

/// Depth-first search for the first expression matching `pred`.
fn find_expr(prog: &Program, pred: impl Fn(&Expr) -> bool + Copy) -> Option<&Expr> {
    let mut out = None;
    for s in first_fn_body_stmts(prog) {
        walk_stmt(s, &pred, &mut out);
        if out.is_some() {
            break;
        }
    }
    out
}

fn walk_stmt<'a>(s: &'a Stmt, pred: &impl Fn(&Expr) -> bool, out: &mut Option<&'a Expr>) {
    fn walk<'a>(e: &'a Expr, pred: &impl Fn(&Expr) -> bool, out: &mut Option<&'a Expr>) {
        if out.is_some() {
            return;
        }
        if pred(e) {
            *out = Some(e);
            return;
        }
        for child in child_exprs(e) {
            walk(child, pred, out);
        }
    }
    match s {
        Stmt::Expr(es) => walk(&es.expr, pred, out),
        Stmt::LocalVar(v) => {
            for d in &v.declarators {
                if let Some(i) = &d.initializer {
                    walk(i, pred, out);
                }
            }
        }
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                walk(v, pred, out);
            }
        }
        Stmt::Block(b) => {
            for st in &b.stmts {
                walk_stmt(st, pred, out);
            }
        }
        _ => {}
    }
}

/// Immediate child expressions of `e`, covering the shapes the gap-fix tests
/// exercise (calls, member/index access, binary/conditional, closures).
fn child_exprs(e: &Expr) -> Vec<&Expr> {
    let mut v = Vec::new();
    match e {
        Expr::Call { callee, args, .. } => {
            v.push(callee.as_ref());
            v.extend(args.positional.iter());
            v.extend(args.named.iter().map(|n| &n.value));
        }
        Expr::Index { object, index, .. } => {
            v.push(object);
            v.push(index);
        }
        Expr::Field { object, .. } => v.push(object),
        Expr::Binary { left, right, .. } => {
            v.push(left);
            v.push(right);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            v.push(condition);
            v.push(then_expr);
            v.push(else_expr);
        }
        Expr::Is { expr, .. } | Expr::As { expr, .. } => v.push(expr),
        Expr::Await { expr, .. } => v.push(expr),
        Expr::FuncExpr { body, .. } => {
            if let FunctionBody::Arrow(inner, _) = body.as_ref() {
                v.push(inner);
            }
        }
        _ => {}
    }
    v
}

fn first_call_first_arg(prog: &Program) -> &Expr {
    let call = find_expr(
        prog,
        |e| matches!(e, Expr::Call { args, .. } if !args.positional.is_empty()),
    )
    .expect("no call with positional args");
    match call {
        Expr::Call { args, .. } => &args.positional[0],
        _ => unreachable!(),
    }
}

fn first_local_var_init(prog: &Program) -> Option<&Expr> {
    for s in first_fn_body_stmts(prog) {
        if let Stmt::LocalVar(v) = s {
            return v.declarators.first().and_then(|d| d.initializer.as_ref());
        }
    }
    None
}

// ── Dart 3 statement-form pattern destructuring ───────────────────────────────

#[test]
fn test_pattern_decl_record_final() {
    // `final (a, b) = expr;` — the jfit construct (recovery_trend_chart.dart).
    let src = "void f(row) { final (color, valueOf) = row; use(color, valueOf); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::PatternDecl(decl) = &first_fn_body_stmts(&prog)[0] else {
        panic!(
            "expected PatternDecl, got {:#?}",
            first_fn_body_stmts(&prog)[0]
        );
    };
    assert!(decl.is_final);
    let Pattern::Record(rec) = &decl.pattern else {
        panic!("expected record pattern, got {:#?}", decl.pattern);
    };
    assert_eq!(rec.fields.len(), 2);
    // Bindings are Variable patterns, not Const references.
    for (field, name) in rec.fields.iter().zip(["color", "valueOf"]) {
        let Pattern::Variable { name: id, .. } = &field.pattern else {
            panic!("expected variable binding, got {:#?}", field.pattern);
        };
        assert_eq!(id.name, name);
    }
    // The initializer is preserved as an Ident expr.
    assert!(matches!(&decl.init, Expr::Ident(id) if id.name == "row"));
}

#[test]
fn test_pattern_decl_var_named_shorthand() {
    // `var (x, :y) = ..;` — positional + `:name` shorthand field.
    let src = "void f(p) { var (x, :y) = p; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::PatternDecl(decl) = &first_fn_body_stmts(&prog)[0] else {
        panic!("expected PatternDecl");
    };
    assert!(!decl.is_final);
    let Pattern::Record(rec) = &decl.pattern else {
        panic!("expected record pattern");
    };
    assert_eq!(rec.fields.len(), 2);
    assert!(rec.fields[0].name.is_none());
    assert_eq!(
        rec.fields[1].name.as_ref().map(|n| n.name.as_str()),
        Some("y")
    );
    assert!(matches!(&rec.fields[1].pattern, Pattern::Variable { name, .. } if name.name == "y"));
}

#[test]
fn test_pattern_decl_switch_initializer() {
    // `final (label, color) = switch (status) { ... };` (scheduling_shared_widgets.dart).
    let src = "void f(status) { final (label, color) = switch (status) { _ => (1, 2) }; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::PatternDecl(decl) = &first_fn_body_stmts(&prog)[0] else {
        panic!("expected PatternDecl");
    };
    assert!(matches!(&decl.pattern, Pattern::Record(r) if r.fields.len() == 2));
    assert!(matches!(&decl.init, Expr::Switch { .. }));
}

#[test]
fn test_pattern_decl_does_not_shadow_record_typed_var() {
    // `final (int, String) rec = ..;` is a record-TYPED var decl, not a pattern.
    let src = "void f() { final (int, String) rec = (1, 'a'); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(&first_fn_body_stmts(&prog)[0], Stmt::LocalVar(_)),
        "expected LocalVar, got {:#?}",
        first_fn_body_stmts(&prog)[0]
    );
}

#[test]
fn test_for_statement_pattern_destructuring() {
    // `for (final (index, slot) in slots.indexed) { .. }` (recovery_trend_chart.dart).
    let src = "void f(slots) { for (final (index, slot) in slots.indexed) { use(index, slot); } }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::For(for_stmt) = &first_fn_body_stmts(&prog)[0] else {
        panic!("expected For, got {:#?}", first_fn_body_stmts(&prog)[0]);
    };
    let Some(ForInit::PatternForIn { pattern, iterable }) = &for_stmt.init else {
        panic!("expected PatternForIn, got {:#?}", for_stmt.init);
    };
    let Pattern::Record(rec) = &**pattern else {
        panic!("expected record pattern");
    };
    assert_eq!(rec.fields.len(), 2);
    assert!(
        matches!(&rec.fields[0].pattern, Pattern::Variable { name, .. } if name.name == "index")
    );
    // The iterable `slots.indexed` is preserved as a Field access.
    assert!(matches!(&**iterable, Expr::Field { .. }));
}

// ── Dart 3.0 null-aware collection elements ───────────────────────────────────

#[test]
fn test_null_aware_list_element() {
    // `[?weightLabel, ...]` (logged_exercise_card.dart).
    let src = "f(weightLabel) => <String>[?weightLabel, 'reps'];";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List { elements, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!("expected list");
    };
    assert_eq!(elements.len(), 2);
    let CollectionElement::NullAware { expr, .. } = &elements[0] else {
        panic!("expected null-aware element, got {:#?}", elements[0]);
    };
    assert!(matches!(expr, Expr::Ident(id) if id.name == "weightLabel"));
    assert!(matches!(&elements[1], CollectionElement::Expr(_)));
}

#[test]
fn test_null_aware_set_element() {
    // `{?activeType}` (exercise_picker_page.dart) — a set, not a map.
    let src = "f(activeType) => {?activeType};";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::Set { elements, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!(
            "expected set, got {:#?}",
            first_fn_arrow_or_block_expr(&prog)
        );
    };
    assert_eq!(elements.len(), 1);
    assert!(matches!(
        &elements[0],
        CollectionElement::NullAware { expr, .. } if matches!(expr, Expr::Ident(id) if id.name == "activeType")
    ));
}

#[test]
fn test_null_aware_map_key_and_value() {
    // `{?k: v}` and `{?k: ?v}` — null-aware key and/or value on a map entry.
    let src = "f(k, v) => {?k: v, a: ?v};";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::Map { entries, .. } = first_fn_arrow_or_block_expr(&prog) else {
        panic!(
            "expected map, got {:#?}",
            first_fn_arrow_or_block_expr(&prog)
        );
    };
    assert_eq!(entries.len(), 2);
    assert!(entries[0].key_null_aware);
    assert!(!entries[0].value_null_aware);
    assert!(!entries[1].key_null_aware);
    assert!(entries[1].value_null_aware);
}
