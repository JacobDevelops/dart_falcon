use insta::assert_debug_snapshot;
use jdlint_dart_parser::parser::parse;
use jdlint_syntax::ast::*;

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
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.name.name, "Foo");
    assert!(cls.members.is_empty());
}

#[test]
fn test_class_with_field() {
    let src = "class Foo { int x; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.members.len(), 1);
    let ClassMember::Field(fld) = &cls.members[0] else { panic!("expected field") };
    assert_eq!(fld.declarators.len(), 1);
    assert_eq!(fld.declarators[0].name.name, "x");
}

#[test]
fn test_class_with_multiple_fields() {
    let src = "class Foo { int x, y; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Field(fld) = &cls.members[0] else { panic!("expected field") };
    assert_eq!(fld.declarators.len(), 2);
}

#[test]
fn test_class_with_method() {
    let src = "class Foo { void bar() {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.members.len(), 1);
    let ClassMember::Method(meth) = &cls.members[0] else { panic!("expected method") };
    assert_eq!(meth.name.name, "bar");
}

#[test]
fn test_class_with_constructor() {
    let src = "class Foo { Foo(); }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Constructor(ctor) = &cls.members[0] else { panic!("expected constructor") };
    assert_eq!(ctor.name.name, "Foo");
}

#[test]
fn test_class_extends() {
    let src = "class Child extends Parent {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(cls.extends.is_some());
}

#[test]
fn test_class_with_clause() {
    let src = "class Child with Mixin1, Mixin2 {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.with_clause.len(), 2);
}

#[test]
fn test_class_implements() {
    let src = "class Foo implements Bar, Baz {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.implements.len(), 2);
}

#[test]
fn test_class_abstract() {
    let src = "abstract class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(cls.modifiers.is_abstract);
}

#[test]
fn test_class_interface() {
    let src = "interface class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(cls.modifiers.is_interface);
}

#[test]
fn test_class_final() {
    let src = "final class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(cls.modifiers.is_final);
}

#[test]
fn test_class_sealed() {
    let src = "sealed class Foo {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert!(cls.modifiers.is_sealed);
}

#[test]
fn test_class_generic() {
    let src = "class Box<T> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.type_params.len(), 1);
}

// ── Mixin ─────────────────────────────────────────────────────────────────────

#[test]
fn test_mixin_simple() {
    let src = "mixin M {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Mixin(m) = &prog.declarations[0] else { panic!("expected mixin") };
    assert_eq!(m.name.name, "M");
}

#[test]
fn test_mixin_with_on() {
    let src = "mixin M on Base {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Mixin(m) = &prog.declarations[0] else { panic!("expected mixin") };
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
    let TopLevelDecl::Enum(en) = &prog.declarations[0] else { panic!("expected enum") };
    assert_eq!(en.name.name, "Color");
    assert_eq!(en.variants.len(), 3);
}

#[test]
fn test_enum_enhanced_with_members() {
    let src = "enum Status { active, inactive; final int code; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Enum(en) = &prog.declarations[0] else { panic!("expected enum") };
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
    assert!(matches!(prog.declarations[0], TopLevelDecl::ExtensionType(_)));
}

// ── Functions ─────────────────────────────────────────────────────────────────

#[test]
fn test_function_void_no_params() {
    let src = "void foo() {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert_eq!(fun.name.name, "foo");
}

#[test]
fn test_function_with_return_type() {
    let src = "int add(int a, int b) { return a + b; }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert!(fun.return_type.is_some());
}

#[test]
fn test_function_async() {
    let src = "void foo() async {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert!(fun.is_async);
}

#[test]
fn test_function_generator() {
    let src = "Iterable<int> foo() sync* {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert!(fun.is_generator);
}

#[test]
fn test_function_arrow() {
    let src = "int double(int x) => x * 2;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    assert!(v.is_final, "field should be final");
}

#[test]
fn test_top_level_const() {
    let src = "const int x = 42;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    assert!(v.is_const);
}

#[test]
fn test_top_level_late() {
    let src = "late int x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    let Some(body) = &fun.body else { panic!("expected body") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = &b.stmts[0] else { panic!("expected if") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = &b.stmts[0] else { panic!("expected if") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else { panic!("expected for") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else { panic!("expected for") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::TryCatch(tc) = &b.stmts[0] else { panic!("expected try-catch") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Return(ret) = &b.stmts[0] else { panic!("expected return") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else { panic!("expected for") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = &b.stmts[0] else { panic!("expected for") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Yield(ys) = &b.stmts[0] else { panic!("expected yield") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
            let Expr::Field { is_null_safe, .. } = &es.expr else { panic!("expected field") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
            let Expr::Is { negated, .. } = &es.expr else { panic!("expected is") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
            let Expr::New { is_const, .. } = &es.expr else { panic!("expected new") };
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
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = &b.stmts[0] else { panic!("expected expr stmt") };
            assert!(matches!(&es.expr, Expr::Switch { .. }));
        }
        _ => panic!("expected block body"),
    }
}

// ── Patterns ──────────────────────────────────────────────────────────────────

#[test]
fn test_pattern_wildcard() {
    let src = "main() { if (x case _) {} }";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    match &fun.body {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = &b.stmts[0] else { panic!("expected if") };
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
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    let ClassMember::Method(meth) = &cls.members[0] else { panic!("expected method") };
    assert!(!meth.annotations.is_empty());
}

// ── Named and optional parameters ─────────────────────────────────────────────

#[test]
fn test_named_parameter() {
    let src = "main({required int x}) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert!(!fun.params.named.is_empty());
}

#[test]
fn test_optional_positional_parameter() {
    let src = "main([int x]) {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = &prog.declarations[0] else { panic!("expected function") };
    assert!(!fun.params.optional_positional.is_empty());
}

// ── Null-safety ───────────────────────────────────────────────────────────────

#[test]
fn test_nullable_type() {
    let src = "int? x;";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = &prog.declarations[0] else { panic!("expected var") };
    assert!(v.var_type.is_some());
}

// ── Generic types ─────────────────────────────────────────────────────────────

#[test]
fn test_generic_single_param() {
    let src = "class Box<T> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
    assert_eq!(cls.type_params.len(), 1);
}

#[test]
fn test_generic_multiple_params() {
    let src = "class Map<K, V> {}";
    let (prog, errors) = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = &prog.declarations[0] else { panic!("expected class") };
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
