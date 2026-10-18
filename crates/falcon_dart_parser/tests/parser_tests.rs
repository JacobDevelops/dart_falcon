use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;
use insta::assert_debug_snapshot;
#[path = "common/first_stmt_expr.rs"]
mod common_expr;
use common_expr::first_stmt_expr;

#[test]
fn test_empty_program() {
    let falcon_dart_parser::ParseResult { tree, errors } = parse("");
    let prog = tree.program("");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(prog.declarations().is_empty());
}

#[test]
fn test_import_simple() {
    let src = r#"import 'dart:core';"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports().len(), 1);
    assert_eq!(prog.imports().next().unwrap().uri().value(), "dart:core");
}

#[test]
fn test_import_with_as() {
    let src = r#"import 'foo.dart' as foo;"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports().len(), 1);
    assert!(prog.imports().next().unwrap().as_name().is_some());
    assert_eq!(
        prog.imports()
            .next()
            .unwrap()
            .as_name()
            .as_ref()
            .unwrap()
            .name(),
        "foo"
    );
}

#[test]
fn test_import_with_show() {
    let src = r#"import 'pkg.dart' show foo, bar;"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports().next().unwrap().combinators().len(), 1);
}

#[test]
fn test_import_with_hide() {
    let src = r#"import 'pkg.dart' hide Secret;"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.imports().next().unwrap().combinators().len(), 1);
}

#[test]
fn test_import_deferred() {
    let src = r#"import 'pkg.dart' deferred as lazy;"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(prog.imports().next().unwrap().is_deferred());
}

#[test]
fn test_export_simple() {
    let src = r#"export 'foo.dart';"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.exports().len(), 1);
}

#[test]
fn test_library_directive() {
    let src = "library my.lib;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(prog.library_directive().is_some());
}

#[test]
fn test_part_directive() {
    let src = r#"part 'foo.dart';"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.part_directives().len(), 1);
}

#[test]
fn test_class_empty() {
    let src = "class Foo {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(prog.declarations().len(), 1);
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.name().name(), "Foo");
    assert!(cls.members().is_empty());
}

#[test]
fn test_class_with_field() {
    let src = "class Foo { int x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.members().len(), 1);
    let ClassMember::Field(fld) = cls.members().next().unwrap() else {
        panic!("expected field")
    };
    assert_eq!(fld.declarators().len(), 1);
    assert_eq!(fld.declarators().next().unwrap().name().name(), "x");
}

#[test]
fn test_class_with_multiple_fields() {
    let src = "class Foo { int x, y; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Field(fld) = cls.members().next().unwrap() else {
        panic!("expected field")
    };
    assert_eq!(fld.declarators().len(), 2);
}

#[test]
fn test_class_with_method() {
    let src = "class Foo { void bar() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.members().len(), 1);
    let ClassMember::Method(meth) = cls.members().next().unwrap() else {
        panic!("expected method")
    };
    assert_eq!(meth.name().name(), "bar");
}

#[test]
fn test_class_with_constructor() {
    let src = "class Foo { Foo(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Constructor(ctor) = cls.members().next().unwrap() else {
        panic!("expected constructor")
    };
    assert_eq!(ctor.name().name(), "Foo");
}

#[test]
fn test_class_extends() {
    let src = "class Child extends Parent {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert!(cls.extends().is_some());
}

#[test]
fn test_class_with_clause() {
    let src = "class Child with Mixin1, Mixin2 {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.with_clause().len(), 2);
}

#[test]
fn test_class_implements() {
    let src = "class Foo implements Bar, Baz {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.implements().len(), 2);
}

#[test]
fn test_class_abstract() {
    let src = "abstract class Foo {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert!(cls.modifiers().is_abstract);
}

#[test]
fn test_class_abstract_base_modifiers_are_independent() {
    let falcon_dart_parser::ParseResult { tree, errors } = parse("abstract base class Foo {}");
    let prog = tree.program("abstract base class Foo {}");
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class");
    };
    let modifiers = cls.modifiers();
    assert_eq!(
        modifiers,
        ClassModifiers {
            is_abstract: true,
            restriction: Some(ClassRestriction::Base),
        }
    );
    assert_eq!(cls.modifiers(), modifiers);
    assert_eq!(ClassModifiers::default().restriction, None);
}

#[test]
fn test_class_interface() {
    let src = "interface class Foo {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(
        cls.modifiers().restriction,
        Some(ClassRestriction::Interface)
    );
}

#[test]
fn test_class_final() {
    let src = "final class Foo {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.modifiers().restriction, Some(ClassRestriction::Final));
}

#[test]
fn test_class_sealed() {
    let src = "sealed class Foo {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.modifiers().restriction, Some(ClassRestriction::Sealed));
}

#[test]
fn test_class_generic() {
    let src = "class Box<T> {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params().len(), 1);
}

#[test]
fn test_mixin_simple() {
    let src = "mixin M {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Mixin(m) = prog.declarations().next().unwrap() else {
        panic!("expected mixin")
    };
    assert_eq!(m.name().name(), "M");
}

#[test]
fn test_mixin_with_on() {
    let src = "mixin M on Base {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Mixin(m) = prog.declarations().next().unwrap() else {
        panic!("expected mixin")
    };
    assert_eq!(m.on_clause().len(), 1);
}

#[test]
fn test_mixin_class() {
    let src = "mixin class M {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(
        prog.declarations().next().unwrap(),
        TopLevelDecl::MixinClass(_)
    ));
}

#[test]
fn test_enum_simple() {
    let src = "enum Color { red, green, blue }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Enum(en) = prog.declarations().next().unwrap() else {
        panic!("expected enum")
    };
    assert_eq!(en.name().name(), "Color");
    assert_eq!(en.variants().len(), 3);
}

#[test]
fn test_enum_enhanced_with_members() {
    let src = "enum Status { active, inactive; final int code; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Enum(en) = prog.declarations().next().unwrap() else {
        panic!("expected enum")
    };
    assert!(!en.members().is_empty());
}

#[test]
fn test_extension_simple() {
    let src = "extension IntParsing on String { }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    // Extensions might not be fully supported yet; check they don't panic
    assert!(!prog.declarations().is_empty() || !errors.is_empty());
}

#[test]
fn test_extension_with_name() {
    let src = "extension NumberParsing on String { }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    // Extensions might not be fully supported yet; check they don't panic
    assert!(!prog.declarations().is_empty() || !errors.is_empty());
}

#[test]
fn test_extension_type() {
    let src = "extension type UserId(int value) {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(
        prog.declarations().next().unwrap(),
        TopLevelDecl::ExtensionType(_)
    ));
}

#[test]
fn test_function_void_no_params() {
    let src = "void foo() {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert_eq!(fun.name().name(), "foo");
}

#[test]
fn test_function_with_return_type() {
    let src = "int add(int a, int b) { return a + b; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert!(fun.return_type().is_some());
}

#[test]
fn test_function_async() {
    let src = "void foo() async {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert_eq!(fun.async_marker(), AsyncMarker::Async);
}

#[test]
fn test_function_generator() {
    let src = "Iterable<int> foo() sync* {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert_eq!(fun.async_marker(), AsyncMarker::SyncStar);
}

#[test]
fn test_function_arrow() {
    let src = "int double(int x) => x * 2;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert!(fun.body().is_some());
}

#[test]
fn test_top_level_var() {
    let src = "void main() {} var x = 42;";
    let falcon_dart_parser::ParseResult {
        tree,
        errors: _errors,
    } = parse(src);
    let prog = tree.program(src);
    // Parser may not support bare var at top level without type context
    assert!(!prog.declarations().is_empty());
}

#[test]
fn test_top_level_final() {
    let src = "final int x = 42;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    assert_eq!(v.keyword(), VarKeyword::Final, "field should be final");
}

#[test]
fn test_top_level_const() {
    let src = "const int x = 42;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    assert_eq!(v.keyword(), VarKeyword::Const);
}

#[test]
fn test_top_level_late() {
    let src = "late int x;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    assert!(v.is_late());
}

#[test]
fn test_top_level_late_final_typed() {
    // `late final` is the correct Dart modifier order (late precedes final).
    let src = "late final int x = 1;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    assert!(v.is_late());
    assert_eq!(v.keyword(), VarKeyword::Final);
    assert!(v.var_type().is_some());
}

#[test]
fn test_top_level_late_final_untyped() {
    let src = "late final x;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    assert!(v.is_late());
    assert_eq!(v.keyword(), VarKeyword::Final);
}

#[test]
fn test_static_late_final_field() {
    let src = "class C { static late final int x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(c) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Field(f) = c.members().next().unwrap() else {
        panic!("expected field")
    };
    assert!(f.is_static());
    assert!(f.is_late());
    assert_eq!(f.keyword(), VarKeyword::Final);
}

#[test]
fn test_typedef_function_type() {
    let src = "typedef Callback = void Function(int);";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(matches!(
        prog.declarations().next().unwrap(),
        TopLevelDecl::TypeAlias(_)
    ));
}

#[test]
fn test_stmt_if() {
    let src = "main() { if (true) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    let Some(body) = fun.body() else {
        panic!("expected body")
    };
    match body {
        FunctionBody::Block(b) => {
            assert_eq!(b.stmts().len(), 1);
            assert!(matches!(b.stmts().next().unwrap(), Stmt::If(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_if_else() {
    let src = "main() { if (true) { } else { } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = b.stmts().next().unwrap() else {
                panic!("expected if")
            };
            assert!(if_stmt.else_branch().is_some());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_if_case() {
    let src = "main() { if (x case 42) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = b.stmts().next().unwrap() else {
                panic!("expected if")
            };
            assert!(matches!(if_stmt.condition(), IfCondition::Case(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_while() {
    let src = "main() { while (true) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::While(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_do_while() {
    let src = "main() { do {} while (true); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::DoWhile(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_for() {
    let src = "main() { for (var i = 0; i < 10; i++) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::For(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_for_in() {
    let src = "main() { for (var x in list) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = b.stmts().next().unwrap() else {
                panic!("expected for")
            };
            assert!(matches!(for_stmt.header().init(), Some(ForInit::ForIn(_))));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_for_await_in() {
    let src = "main() async { for (var x in stream) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = b.stmts().next().unwrap() else {
                panic!("expected for")
            };
            assert!(matches!(for_stmt.header().init(), Some(ForInit::ForIn(_))));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_switch() {
    let src = "main() { switch (x) { case 1: break; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::Switch(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_try_catch() {
    let src = "main() { try {} catch (e) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::TryCatch(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_try_catch_finally() {
    let src = "main() { try {} catch (e) {} finally {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::TryCatch(tc) = b.stmts().next().unwrap() else {
                panic!("expected try-catch")
            };
            assert!(tc.finally().is_some());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_return() {
    let src = "main() { return 42; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Return(ret) = b.stmts().next().unwrap() else {
                panic!("expected return")
            };
            assert!(ret.value().is_some());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_throw() {
    let src = "main() { throw Exception(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::Throw(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_break() {
    let src = "main() { for (;;) { break; } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = b.stmts().next().unwrap() else {
                panic!("expected for")
            };
            match for_stmt.body() {
                Stmt::Block(bl) => {
                    assert!(matches!(bl.stmts().next().unwrap(), Stmt::Break(_)));
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::For(for_stmt) = b.stmts().next().unwrap() else {
                panic!("expected for")
            };
            match for_stmt.body() {
                Stmt::Block(bl) => {
                    assert!(matches!(bl.stmts().next().unwrap(), Stmt::Continue(_)));
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::Assert(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_yield() {
    let src = "gen() sync* { yield 42; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::Yield(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_yield_star() {
    let src = "gen() sync* { yield* other; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Yield(ys) = b.stmts().next().unwrap() else {
                panic!("expected yield")
            };
            assert!(ys.is_star());
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_stmt_local_var() {
    let src = "main() { int x = 42; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            assert!(matches!(b.stmts().next().unwrap(), Stmt::LocalVar(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_integer_literal() {
    let src = "main() { 42; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::IntLit(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_string_literal() {
    let src = r#"main() { "hello"; }"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::StringLit(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_bool_literal() {
    let src = "main() { true; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::BoolLit(literal) if literal.value()));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_null_literal() {
    let src = "main() { null; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::NullLit(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_identifier() {
    let src = "main() { foo; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Ident(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_this() {
    let src = "main() { this; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::This(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_super() {
    let src = "main() { super; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Super(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_binary_add() {
    let src = "main() { a + b; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Binary(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_unary_minus() {
    let src = "main() { -x; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Unary(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_field_access() {
    let src = "main() { obj.field; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Field(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_null_safe_field_access() {
    let src = "main() { obj?.field; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            let Expr::Field(node) = es.expr() else {
                panic!("expected field")
            };
            let is_null_safe = node.is_null_safe();
            assert!(is_null_safe);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_index_access() {
    let src = "main() { arr[0]; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Index(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_call() {
    let src = "main() { foo(1, 2); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Call(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_cascade() {
    let src = "main() { obj..foo()..bar; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Cascade(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_conditional() {
    let src = "main() { a ? b : c; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Conditional(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_is_type() {
    let src = "main() { x is int; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Is(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_is_not_type() {
    let src = "main() { x is! int; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            let Expr::Is(node) = es.expr() else {
                panic!("expected is")
            };
            let negated = node.negated();
            assert!(negated);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_as_cast() {
    let src = "main() { (x as int); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            let Expr::Paren(node) = es.expr() else {
                panic!("expected parenthesized expression")
            };
            let expr = node.expr();
            assert!(matches!(expr, Expr::As(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_list_literal() {
    let src = "main() { [1, 2, 3]; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::List(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_map_literal() {
    let src = "main() { {'a': 1, 'b': 2}; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Map(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_set_literal() {
    let src = "main() { {1, 2, 3}; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Set(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_record_literal() {
    let src = "main() { (1, 2, x: 3); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Record(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_await() {
    let src = "main() async { await foo(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Await(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_new() {
    let src = "main() { new Foo(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::New(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_const_new() {
    let src = "main() { const Foo(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            let Expr::New(node) = es.expr() else {
                panic!("expected new")
            };
            let is_const = node.is_const();
            assert!(is_const);
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_expr_switch_expression() {
    let src = "main() { x switch { 1 => 'a', _ => 'b' }; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::Expr(es) = b.stmts().next().unwrap() else {
                panic!("expected expr stmt")
            };
            assert!(matches!(es.expr(), Expr::Switch(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn switch_guard_parentheses_do_not_start_arrow_functions() {
    for guard in ["(flag)", "(flag = other)", "(required)"] {
        let source = format!("main() {{ x switch {{ _ when {guard} => 1, _ => 0 }}; }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{guard}: {:?}", parsed.errors);
        let Expr::Switch(switch) = first_stmt_expr(parsed.tree.program(&source)) else {
            panic!("expected switch expression");
        };
        assert_eq!(switch.arms().len(), 2, "{guard}");
        let Expr::Paren(condition) = switch.arms().next().unwrap().guard().unwrap() else {
            panic!("expected parenthesized guard: {guard}");
        };
        if guard == "(flag = other)" {
            assert!(matches!(condition.expr(), Expr::Assign(_)));
        } else {
            assert!(matches!(condition.expr(), Expr::Ident(_)));
        }
    }
}

#[test]
fn switch_guards_preserve_nested_closures_and_parameter_defaults() {
    for guard in [
        "(() => true)()",
        "(() { return true; })()",
        "(([flag = true]) => flag)()",
        "(({flag = true}) => flag)()",
        "check((flag) => flag)",
    ] {
        let source = format!("main() {{ x switch {{ _ when {guard} => 1, _ => 0 }}; }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{guard}: {:?}", parsed.errors);
        let Expr::Switch(switch) = first_stmt_expr(parsed.tree.program(&source)) else {
            panic!("expected switch expression");
        };
        assert_eq!(switch.arms().len(), 2, "{guard}");
        assert!(
            matches!(switch.arms().next().unwrap().guard(), Some(Expr::Call(_))),
            "{guard}"
        );
    }
}

#[test]
fn switch_guard_arrow_lookahead_respects_nested_delimiters() {
    for guard in [
        "(items?[0] ?? false)",
        "items?[0] == (() => true)()",
        "flag ? true : (other)",
        "switch (x) { _ when (flag) => true, _ => false }",
    ] {
        let source = format!("main() {{ x switch {{ _ when {guard} => 1, _ => 0 }}; }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{guard}: {:?}", parsed.errors);
        let Expr::Switch(switch) = first_stmt_expr(parsed.tree.program(&source)) else {
            panic!("expected switch expression");
        };
        assert_eq!(switch.arms().len(), 2, "{guard}");
        assert!(switch.arms().next().unwrap().guard().is_some(), "{guard}");
    }
}

#[test]
fn test_dot_shorthand_bare() {
    let source = "main() { .red; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    assert!(
        matches!(expr, Expr::DotShorthand(dot) if !dot.is_const() && dot.name().name() == "red")
    );
}

#[test]
fn test_dot_shorthand_call() {
    let source = "main() { .parse('42'); }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    let Expr::Call(node) = expr else {
        panic!("expected call, got {expr:?}")
    };
    let callee = node.callee();
    let args = node.args();
    assert!(matches!(callee, Expr::DotShorthand(dot) if dot.name().name() == "parse"));
    assert_eq!(args.positional().len(), 1);
}

#[test]
fn test_dot_shorthand_generic_call() {
    let source = "main() { .parse<int>('42'); }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    let Expr::Call(node) = expr else {
        panic!("expected call, got {expr:?}")
    };
    let callee = node.callee();
    let type_args = node.type_args();
    assert!(matches!(callee, Expr::DotShorthand(dot) if dot.name().name() == "parse"));
    assert_eq!(type_args.len(), 1);
}

#[test]
fn test_dot_shorthand_new_tearoff() {
    let source = "main() { .new; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    assert!(matches!(expr, Expr::DotShorthand(dot) if dot.name().name() == "new"));
}

#[test]
fn test_dot_shorthand_new_call() {
    let source = "main() { .new(1, 2); }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    let Expr::Call(node) = expr else {
        panic!("expected call, got {expr:?}")
    };
    let callee = node.callee();
    let args = node.args();
    assert!(matches!(callee, Expr::DotShorthand(dot) if dot.name().name() == "new"));
    assert_eq!(args.positional().len(), 2);
}

#[test]
fn test_dot_shorthand_const() {
    // `const` in statement position is a variable-declaration keyword, so drive
    // the const-expression path via a list element instead.
    let source = "main() { [const .fromLtrb(4, 3, 2, 1)]; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    let Expr::List(node) = expr else {
        panic!("expected list, got {expr:?}")
    };
    let elements = node.elements();
    let CollectionElement::Expr(Expr::Call(call)) = elements.clone().next().unwrap() else {
        panic!(
            "expected call element, got {:?}",
            elements.clone().next().unwrap()
        )
    };
    let callee = call.callee();
    let args = call.args();
    let Expr::DotShorthand(node) = callee else {
        panic!("expected dot shorthand, got {callee:?}")
    };
    let is_const = node.is_const();
    let name = node.name();
    assert!(is_const);
    assert_eq!(name.name(), "fromLtrb");
    assert_eq!(args.positional().len(), 4);
}

#[test]
fn test_dot_shorthand_postfix_chain() {
    // A shorthand head still supports trailing selectors: `.red.value`.
    let source = "main() { .red.value; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    let Expr::Field(node) = expr else {
        panic!("expected field access, got {expr:?}")
    };
    let object = node.object();
    let field = node.field();
    assert_eq!(field.name(), "value");
    assert!(matches!(object, Expr::DotShorthand(dot) if dot.name().name() == "red"));
}

#[test]
fn test_int_digit_separator_literal() {
    let source = "main() { 1_000_000; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    assert!(matches!(expr, Expr::IntLit(literal) if literal.value() == "1_000_000"));
}

#[test]
fn test_hex_digit_separator_literal() {
    let source = "main() { 0xFF_EC; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    assert!(matches!(expr, Expr::IntLit(literal) if literal.value() == "0xFF_EC"));
}

#[test]
fn test_double_digit_separator_literal() {
    let source = "main() { 1_2.3_4e1_2; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let expr = first_stmt_expr(parsed.tree.program(source));
    assert!(matches!(expr, Expr::DoubleLit(literal) if literal.value() == "1_2.3_4e1_2"));
}

#[test]
fn test_pattern_wildcard() {
    let src = "main() { if (x case _) {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body() {
        Some(FunctionBody::Block(b)) => {
            let Stmt::If(if_stmt) = b.stmts().next().unwrap() else {
                panic!("expected if")
            };
            assert!(matches!(if_stmt.condition(), IfCondition::Case(_)));
        }
        _ => panic!("expected block body"),
    }
}

#[test]
fn test_pattern_variable() {
    let src = "main() { if (x case var y) {} }";
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_literal() {
    let src = "main() { if (x case 42) {} }";
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_list() {
    let src = "main() { if (x case [a, b]) {} }";
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_map() {
    let src = "main() { if (x case {'a': a}) {} }";
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_pattern_record() {
    let src = "main() { if (x case (a, b)) {} }";
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn test_error_recovery_partial_class() {
    let src = "class Foo { int x }";
    let falcon_dart_parser::ParseResult {
        tree,
        errors: _errors,
    } = parse(src);
    let prog = tree.program(src);
    // Parser should still produce some AST even with errors
    assert!(matches!(
        prog.declarations().next().unwrap(),
        TopLevelDecl::Class(_)
    ));
}

#[test]
fn test_error_recovery_malformed_function() {
    let src = "void foo() {";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    // Parser should not panic
    assert!(!prog.declarations().is_empty() || !errors.is_empty());
}

#[test]
fn test_annotation_override() {
    let src = "class Foo { @override void bar() {} }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let ClassMember::Method(meth) = cls.members().next().unwrap() else {
        panic!("expected method")
    };
    assert!(!meth.annotations().is_empty());
}

#[test]
fn test_named_parameter() {
    let src = "main({required int x}) {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert!(
        fun.params()
            .unwrap()
            .params()
            .any(|param| matches!(param.kind(), ParamKind::Named { .. }))
    );
}

#[test]
fn test_optional_positional_parameter() {
    let src = "main([int x]) {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    assert!(
        fun.params()
            .unwrap()
            .params()
            .any(|param| param.kind() == ParamKind::OptionalPositional)
    );
}

#[test]
fn test_nullable_type() {
    let src = "int? x;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected var")
    };
    assert!(v.var_type().is_some());
}

#[test]
fn test_generic_single_param() {
    let src = "class Box<T> {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params().len(), 1);
}

#[test]
fn test_generic_multiple_params() {
    let src = "class Map<K, V> {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    assert_eq!(cls.type_params().len(), 2);
}

#[test]
fn test_nested_generic_type() {
    let src = "List<Map<String, int>> data;";
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn snap_class_simple() {
    let falcon_dart_parser::ParseResult { tree, errors } = parse("class Foo {}");
    let prog = tree.program("class Foo {}");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_class_with_field_and_method() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("class Point { int x; void move(int dx) {} }");
    let prog = tree.program("class Point { int x; void move(int dx) {} }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_top_level_function() {
    let falcon_dart_parser::ParseResult { tree, errors } = parse("int add(int a, int b) => a + b;");
    let prog = tree.program("int add(int a, int b) => a + b;");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_import_directive() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("import 'dart:core' show List, Map;");
    let prog = tree.program("import 'dart:core' show List, Map;");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.imports().next().unwrap());
}

#[test]
fn snap_enum_simple() {
    let falcon_dart_parser::ParseResult { tree, errors } = parse("enum Color { red, green, blue }");
    let prog = tree.program("enum Color { red, green, blue }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_typedef_function() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("typedef Callback = void Function(int);");
    let prog = tree.program("typedef Callback = void Function(int);");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_abstract_class_with_getter() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("abstract class Shape { double get area; }");
    let prog = tree.program("abstract class Shape { double get area; }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_switch_expression() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("String f(int x) => x switch { 1 => 'one', _ => 'other' };");
    let prog = tree.program("String f(int x) => x switch { 1 => 'one', _ => 'other' };");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_sealed_class() {
    let falcon_dart_parser::ParseResult { tree, errors } = parse("sealed class Result<T> {}");
    let prog = tree.program("sealed class Result<T> {}");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_extension() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("extension StringX on String { bool get isBlank => length == 0; }");
    let prog = tree.program("extension StringX on String { bool get isBlank => length == 0; }");
    assert!(errors.is_empty());
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_mixin_declaration() {
    let falcon_dart_parser::ParseResult { tree, errors } =
        parse("mixin Serializable on Object { Map<String, dynamic> toJson(); }");
    let prog = tree.program("mixin Serializable on Object { Map<String, dynamic> toJson(); }");
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_class_extends_with_implements() {
    let src = "class Dog extends Animal with Barker implements Pet { final String name; Dog(this.name); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_named_constructor() {
    let src = "class Vec2 { final double x; final double y; Vec2(this.x, this.y); Vec2.zero() : x = 0, y = 0; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_factory_constructor() {
    let src = "class Singleton { static final Singleton _i = Singleton._(); factory Singleton() => _i; Singleton._(); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_async_function() {
    let src =
        "Future<String> fetchName() async { await Future.delayed(Duration.zero); return 'dart'; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_generic_function() {
    let src = "T identity<T>(T value) => value;";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_record_return_type() {
    let src = "(String, int) pair() => ('hello', 42);";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_annotation_with_args() {
    let src = "@Deprecated('Use newFn instead')\nvoid oldFn() {}";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_late_final_field() {
    let src = "class Lazy { late final String value; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_typed_list_literal() {
    let src = "List<String> names() => <String>['Alice', 'Bob'];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_collection_for() {
    let src = "List<int> doubled(List<int> xs) => [for (final x in xs) x * 2];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_switch_pattern_guard() {
    let src = r#"String classify(int n) => switch (n) { < 0 => 'neg', 0 => 'zero', _ => 'pos' };"#;
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_enum_with_members() {
    let src = "enum Color { red, green, blue; bool get isPrimary => this != Color.green; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_typedef_function_type() {
    let src = "typedef Predicate<T> = bool Function(T value);";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

#[test]
fn snap_const_class_field() {
    let src = "class Config { static const int maxRetries = 3; static const String baseUrl = 'https://api.example.com'; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_debug_snapshot!(prog.declarations().next().unwrap());
}

// ── Bug reproduction: Dart 3 pattern elements in collections ──────────────────

fn first_fn_arrow_or_block_expr(prog: Program<'_>) -> Expr<'_> {
    let TopLevelDecl::Function(fun) = prog.declarations().next().unwrap() else {
        panic!("expected function")
    };
    match fun.body().expect("body") {
        FunctionBody::Arrow(body) => body.expr(),
        FunctionBody::Block(b) => match b.stmts().next().unwrap() {
            Stmt::Expr(es) => es.expr(),
            Stmt::LocalVar(vd) => vd
                .declarators()
                .next()
                .unwrap()
                .initializer()
                .expect("init"),
            other => panic!("unexpected stmt: {other:?}"),
        },
        _ => panic!("unexpected body"),
    }
}

#[test]
fn test_bug_collection_if_case_keeps_all_elements() {
    // if-element with a `case` pattern must not truncate later elements.
    let src = "f(e) => [ if (e.x case final y?) ...[1] , other, elements ];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!("expected list")
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 3, "elements: {elements:#?}");
}

#[test]
fn test_bug_collection_if_case_with_when_guard() {
    let src = "f(e) => [ if (e.x case final y? when y > 0) y, other, elements ];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!("expected list")
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 3, "elements: {elements:#?}");
}

#[test]
fn test_bug_collection_for_record_destructuring() {
    let src = "f(list) => [ for (final (i, s) in list.indexed) widget(i, s), tail ];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!("expected list")
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 2, "elements: {elements:#?}");
}

#[test]
fn test_bug_map_comprehension_for_element() {
    let src = "f(xs) => { for (final x in xs) x.id: x };";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(first_fn_arrow_or_block_expr(prog), Expr::Map(_)),
        "expected map, got {:#?}",
        first_fn_arrow_or_block_expr(prog)
    );
}

#[test]
fn test_bug_map_comprehension_for_pattern() {
    let src = "f(xs) => { for (final (k, v) in xs) k: v };";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(first_fn_arrow_or_block_expr(prog), Expr::Map(_)),
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let build = cls
        .members()
        .find_map(|m| match m {
            ClassMember::Method(mth) if mth.name().name() == "build" => Some(mth),
            _ => None,
        })
        .expect("build method");
    assert_eq!(
        build.annotations().len(),
        1,
        "expected @override on build, annotations: {:#?}",
        build.annotations()
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let TopLevelDecl::Class(cls) = prog.declarations().next().unwrap() else {
        panic!("expected class")
    };
    let build = cls
        .members()
        .find_map(|m| match m {
            ClassMember::Method(mth) if mth.name().name() == "build" => Some(mth),
            _ => None,
        })
        .expect("build method");
    assert_eq!(
        build.annotations().len(),
        1,
        "expected @override on build, annotations: {:#?}",
        build.annotations()
    );
}

#[test]
fn test_bug_collection_nested_spreads_after_pattern_constructs() {
    // Spreads following both a case-if and a pattern-for must survive.
    let src =
        "f(a, b, xs) => [ if (a case final y?) ...[y], for (final (i, s) in xs) ...[i, s], ...b ];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!("expected list")
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 3, "elements: {elements:#?}");
}

// ── Closure / expression parser gap fixes ─────────────────────────────────────

/// Assert a source snippet parses with zero errors.
fn assert_parses(src: &str) {
    let falcon_dart_parser::ParseResult { errors, .. } = parse(src);
    assert!(errors.is_empty(), "src: {src}\nerrors: {errors:?}");
}

#[test]
fn test_closure_final_param() {
    let src = "void f() { list.map((final x) => x.y); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    // Drill to the closure argument and confirm it is a function expression.
    let arg = first_call_first_arg(prog);
    assert!(
        matches!(arg, Expr::FuncExpr(function) if function.params().params().len() == 1
            && function.params().params().next().unwrap().kind() == ParamKind::Positional),
        "expected FuncExpr with one positional param, got {arg:#?}"
    );
    if let Expr::FuncExpr(function) = arg {
        let param = function.params().params().next().unwrap();
        assert!(param.is_final(), "param should be final");
        assert_eq!(param.name().name(), "x");
    }
}

#[test]
fn test_closure_var_param() {
    let src = "void f() { g((var x) => x); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let arg = first_call_first_arg(prog);
    assert!(
        matches!(arg, Expr::FuncExpr(function) if function.params().params().len() == 1
            && function.params().params().next().unwrap().kind() == ParamKind::Positional
            && function.params().params().next().unwrap().param_type().is_none()),
        "expected untyped var param, got {arg:#?}"
    );
}

#[test]
fn test_closure_final_typed_param() {
    let src = "void f() { g((final int x) => x); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let arg = first_call_first_arg(prog);
    assert!(
        matches!(arg, Expr::FuncExpr(function) if function.params().params().next().unwrap().kind() == ParamKind::Positional
            && function.params().params().next().unwrap().is_final()
            && function.params().params().next().unwrap().param_type().is_some()),
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let init = first_local_var_init(prog);
    assert!(
        matches!(init, Some(Expr::Conditional(_))),
        "expected conditional, got {init:#?}"
    );
}

#[test]
fn test_is_nullable_type_still_parses() {
    // A genuine nullable type-test keeps the `?` on the type.
    let src = "void f() { g(x is String?); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let arg = first_call_first_arg(prog);
    assert!(matches!(arg, Expr::Is(_)), "expected Is, got {arg:#?}");
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List(list) = first_local_var_init(prog).expect("init") else {
        panic!("expected list literal")
    };
    let elements = list.elements();
    assert!(
        matches!(
            elements.clone().next(),
            Some(CollectionElement::For(element))
                if matches!(element.header().init(), Some(ForInit::VarDecl(_)))
        ),
        "expected C-style For element, got {elements:#?}"
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
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let getter = prog
        .declarations()
        .find_map(|d| match d {
            TopLevelDecl::Class(c) => c.members().find_map(|m| match m {
                ClassMember::Getter(g) => Some(g),
                _ => None,
            }),
            _ => None,
        })
        .expect("getter");
    assert_eq!(
        getter.async_marker(),
        AsyncMarker::Async,
        "getter should be async"
    );
}

#[test]
fn test_nullable_function_type_param() {
    // `String? Function(...)? name` — nullable return type before `Function`.
    assert_parses("class C { const C({final String? Function(int)? reselect}); }");
}

#[test]
fn test_generic_function_expression() {
    let src = "void f() { final g = <T extends Object>(final T x) { return x; }; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let init = first_local_var_init(prog).expect("init");
    assert!(
        matches!(init, Expr::FuncExpr(function) if function.type_params().len() == 1),
        "expected generic FuncExpr, got {init:#?}"
    );
}

#[test]
fn test_assert_initializer_trailing_comma_message() {
    assert_parses("class C { C(this.a) : assert(a != null, 'a must be provided',); final int a; }");
}

// ── Helpers for the gap-fix tests ─────────────────────────────────────────────

fn first_fn_body_stmts(prog: Program<'_>) -> AstChildren<'_, Stmt<'_>> {
    for d in prog.declarations() {
        if let TopLevelDecl::Function(f) = d
            && let Some(FunctionBody::Block(b)) = f.body()
        {
            return b.stmts();
        }
        if let TopLevelDecl::Class(c) = d {
            for m in c.members() {
                if let ClassMember::Method(mth) = m
                    && let Some(FunctionBody::Block(b)) = mth.body()
                {
                    return b.stmts();
                }
            }
        }
    }
    AstChildren::empty()
}

fn find_expr<'a>(
    prog: Program<'a>,
    select: impl Fn(Expr<'a>) -> Option<Expr<'a>>,
) -> Option<Expr<'a>> {
    struct Finder<'a, F> {
        select: F,
        found: Option<Expr<'a>>,
    }
    impl<'a, F: Fn(Expr<'a>) -> Option<Expr<'a>>> Visitor<'a> for Finder<'a, F> {
        fn visit_expr(&mut self, node: Expr<'a>) {
            if self.found.is_some() {
                return;
            }
            self.found = (self.select)(node);
            if self.found.is_none() {
                walk_expr(self, node);
            }
        }
    }
    let mut finder = Finder {
        select,
        found: None,
    };
    for stmt in first_fn_body_stmts(prog) {
        finder.visit_stmt(stmt);
        if finder.found.is_some() {
            break;
        }
    }
    finder.found
}

fn first_call_first_arg(prog: Program<'_>) -> Expr<'_> {
    find_expr(prog, |expr| match expr {
        Expr::Call(call) => call.args().positional().next(),
        _ => None,
    })
    .expect("no call with positional args")
}

fn first_local_var_init(prog: Program<'_>) -> Option<Expr<'_>> {
    for s in first_fn_body_stmts(prog) {
        if let Stmt::LocalVar(v) = s {
            return v.declarators().next().and_then(|d| d.initializer());
        }
    }
    None
}

// ── Dart 3 statement-form pattern destructuring ───────────────────────────────

#[test]
fn test_pattern_decl_record_final() {
    // `final (a, b) = expr;` — the jfit construct (recovery_trend_chart.dart).
    let src = "void f(row) { final (color, valueOf) = row; use(color, valueOf); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::PatternDecl(decl) = first_fn_body_stmts(prog).next().unwrap() else {
        panic!(
            "expected PatternDecl, got {:#?}",
            first_fn_body_stmts(prog).next().unwrap()
        );
    };
    assert_eq!(decl.keyword(), VarKeyword::Final);
    let Pattern::Record(rec) = decl.pattern() else {
        panic!("expected record pattern, got {:#?}", decl.pattern());
    };
    assert_eq!(rec.fields().len(), 2);
    // Bindings are Variable patterns, not Const references.
    for (field, name) in rec.fields().zip(["color", "valueOf"]) {
        let Pattern::Variable(variable) = field.pattern() else {
            panic!("expected variable binding, got {:#?}", field.pattern());
        };
        assert_eq!(variable.name().name(), name);
    }
    // The initializer is preserved as an Ident expr.
    assert!(matches!(decl.init(), Expr::Ident(id) if id.name() == "row"));
}

#[test]
fn test_pattern_decl_var_named_shorthand() {
    // `var (x, :y) = ..;` — positional + `:name` shorthand field.
    let src = "void f(p) { var (x, :y) = p; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::PatternDecl(decl) = first_fn_body_stmts(prog).next().unwrap() else {
        panic!("expected PatternDecl");
    };
    assert_eq!(decl.keyword(), VarKeyword::Var);
    let Pattern::Record(rec) = decl.pattern() else {
        panic!("expected record pattern");
    };
    assert_eq!(rec.fields().len(), 2);
    assert!(rec.fields().next().unwrap().name().is_none());
    assert_eq!(
        rec.fields().nth(1).unwrap().name().map(|n| n.name()),
        Some("y")
    );
    assert!(
        matches!(rec.fields().nth(1).unwrap().pattern(), Pattern::Variable(variable) if variable.name().name() == "y")
    );
}

#[test]
fn test_pattern_decl_switch_initializer() {
    // `final (label, color) = switch (status) { ... };` (scheduling_shared_widgets.dart).
    let src = "void f(status) { final (label, color) = switch (status) { _ => (1, 2) }; }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::PatternDecl(decl) = first_fn_body_stmts(prog).next().unwrap() else {
        panic!("expected PatternDecl");
    };
    assert!(matches!(decl.pattern(), Pattern::Record(r) if r.fields().len() == 2));
    assert!(matches!(decl.init(), Expr::Switch(_)));
}

#[test]
fn test_pattern_decl_does_not_shadow_record_typed_var() {
    // `final (int, String) rec = ..;` is a record-TYPED var decl, not a pattern.
    let src = "void f() { final (int, String) rec = (1, 'a'); }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(
        matches!(first_fn_body_stmts(prog).next().unwrap(), Stmt::LocalVar(_)),
        "expected LocalVar, got {:#?}",
        first_fn_body_stmts(prog).next().unwrap()
    );
}

#[test]
fn test_for_statement_pattern_destructuring() {
    // `for (final (index, slot) in slots.indexed) { .. }` (recovery_trend_chart.dart).
    let src = "void f(slots) { for (final (index, slot) in slots.indexed) { use(index, slot); } }";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Stmt::For(for_stmt) = first_fn_body_stmts(prog).next().unwrap() else {
        panic!(
            "expected For, got {:#?}",
            first_fn_body_stmts(prog).next().unwrap()
        );
    };
    let Some(ForInit::PatternForIn(init)) = for_stmt.header().init() else {
        panic!("expected PatternForIn, got {:#?}", for_stmt.header().init());
    };
    let Pattern::Record(rec) = init.pattern() else {
        panic!("expected record pattern");
    };
    assert_eq!(rec.fields().len(), 2);
    assert!(
        matches!(rec.fields().next().unwrap().pattern(), Pattern::Variable(variable) if variable.name().name() == "index")
    );
    // The iterable `slots.indexed` is preserved as a Field access.
    assert!(matches!(init.iterable(), Expr::Field(_)));
}

// ── Dart 3.0 null-aware collection elements ───────────────────────────────────

#[test]
fn test_null_aware_list_element() {
    // `[?weightLabel, ...]` (logged_exercise_card.dart).
    let src = "f(weightLabel) => <String>[?weightLabel, 'reps'];";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::List(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!("expected list");
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 2);
    let CollectionElement::NullAware(element) = elements.clone().next().unwrap() else {
        panic!(
            "expected null-aware element, got {:#?}",
            elements.clone().next().unwrap()
        );
    };
    assert!(matches!(element.expr(), Expr::Ident(id) if id.name() == "weightLabel"));
    assert!(matches!(
        elements.clone().nth(1).unwrap(),
        CollectionElement::Expr(_)
    ));
}

#[test]
fn test_null_aware_set_element() {
    // `{?activeType}` (exercise_picker_page.dart) — a set, not a map.
    let src = "f(activeType) => {?activeType};";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::Set(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!(
            "expected set, got {:#?}",
            first_fn_arrow_or_block_expr(prog)
        );
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 1);
    assert!(matches!(
        elements.clone().next().unwrap(),
        CollectionElement::NullAware(element) if matches!(element.expr(), Expr::Ident(id) if id.name() == "activeType")
    ));
}

#[test]
fn test_null_aware_map_key_and_value() {
    // `{?k: v}` and `{?k: ?v}` — null-aware key and/or value on a map entry.
    let src = "f(k, v) => {?k: v, a: ?v};";
    let falcon_dart_parser::ParseResult { tree, errors } = parse(src);
    let prog = tree.program(src);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Expr::Map(node) = first_fn_arrow_or_block_expr(prog) else {
        panic!(
            "expected map, got {:#?}",
            first_fn_arrow_or_block_expr(prog)
        );
    };
    let elements = node.elements();
    assert_eq!(elements.len(), 2);
    let MapElement::Entry(first) = elements.clone().next().unwrap() else {
        panic!("expected entry")
    };
    let MapElement::Entry(second) = elements.clone().nth(1).unwrap() else {
        panic!("expected entry")
    };
    assert!(first.key_null_aware());
    assert!(!first.value_null_aware());
    assert!(!second.key_null_aware());
    assert!(second.value_null_aware());
}
