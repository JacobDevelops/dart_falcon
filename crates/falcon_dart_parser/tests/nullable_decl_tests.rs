//! Nullable declarations and conditional expression statements share a `T? name` head.

use falcon_dart_parser::parse;
use falcon_syntax::cst::{Expr, Stmt};
use std::sync::mpsc;
use std::time::Duration;

mod common;
use common::body_stmts;

#[test]
fn nullable_declaration_initializers_keep_their_expression_shapes() {
    for statement in [
        "int? x = cond ? a : b;",
        "T? x = cond ? Function() : other;",
        "T? x = value as T?;",
        "T? x = value as T? Function();",
        "T? x = cond ? value as T? : other;",
        "T? x = cond ? f<T? Function()>() : other;",
        "bool? r = i < n ? xs[i] > 0 : false;",
        "T? x = {'key': cond ? a : b};",
        "T? x = call(named: cond ? a : b);",
        "T? x = () { a ? b = 1 : c; };",
        "T? x;",
        "T? x, y = cond ? a : b;",
        "prefix.T<List<int>>? x = cond ? a : b;",
        "(int, String)? x;",
        "int Function()? x;",
    ] {
        let source = format!("void f() {{ {statement} after(); }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        let mut statements = body_stmts(parsed.tree.program(&source));
        assert_eq!(statements.len(), 2, "{source}");
        assert!(
            matches!(statements.next(), Some(Stmt::LocalVar(_))),
            "{source}"
        );
        assert!(matches!(statements.next(), Some(Stmt::Expr(_))), "{source}");
    }

    let source = "void f() { int? x = cond ? a : b; T? y; }";
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut statements = body_stmts(parsed.tree.program(source));
    let Some(Stmt::LocalVar(declaration)) = statements.next() else {
        panic!("expected nullable declaration");
    };
    assert!(matches!(
        declaration.declarators().next().unwrap().initializer(),
        Some(Expr::Conditional(_))
    ));
    let Some(Stmt::LocalVar(declaration)) = statements.next() else {
        panic!("expected uninitialized nullable declaration");
    };
    assert!(
        declaration
            .declarators()
            .next()
            .unwrap()
            .initializer()
            .is_none()
    );
}

#[test]
fn conditional_assignment_statements_are_not_nullable_declarations() {
    for statement in [
        "a ? b = 1 : c;",
        "a ? b = cond ? x : y : c;",
        "a ? b = value as T? : c;",
        "a ? b = value as T? Function() : c;",
        "a ? b = f<T? Function()>() : c;",
        "a ? b = {'key': cond ? x : y} : c;",
        "a ? b = () { T? x; } : c;",
    ] {
        let source = format!("void f() {{ {statement} after(); }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        let mut statements = body_stmts(parsed.tree.program(&source));
        assert_eq!(statements.len(), 2, "{source}");
        let Some(Stmt::Expr(statement)) = statements.next() else {
            panic!("expected conditional expression statement: {source}");
        };
        let Expr::Conditional(conditional) = statement.expr() else {
            panic!("expected conditional expression: {source}");
        };
        assert!(
            matches!(conditional.then_expr(), Expr::Assign(_)),
            "{source}"
        );
    }
}

#[test]
fn forty_nested_ambiguous_closure_statements_finish_without_reparsing_bodies() {
    for (head, tail) in [
        ("a ? b = () {", "} : c;"),
        ("T? x = () {", "};"),
        ("cond ? run(onDone: () {", "}) : skip();"),
        ("cond ? run(() {", "}) : skip();"),
    ] {
        for depth in [20, 40] {
            let source = format!(
                "void f() {{ {} work(); {} after(); }}",
                head.repeat(depth),
                tail.repeat(depth)
            );
            let (send, recv) = mpsc::channel();
            std::thread::spawn(move || {
                let parsed = parse(&source);
                assert!(parsed.tree.is_lossless(&source));
                if depth == 20 {
                    let mut statements = body_stmts(parsed.tree.program(&source));
                    assert_eq!(statements.len(), 2);
                    if head.starts_with("T?") {
                        assert!(matches!(statements.next(), Some(Stmt::LocalVar(_))));
                    } else {
                        assert!(matches!(
                            statements.next(),
                            Some(Stmt::Expr(statement))
                                if matches!(statement.expr(), Expr::Conditional(_))
                        ));
                    }
                }
                send.send(parsed.errors).unwrap();
            });
            let errors = recv
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|error| {
                    panic!("nested head {head:?} at depth {depth} must finish within ten seconds: {error}")
                });
            if depth == 20 {
                assert!(errors.is_empty(), "{errors:?}");
            } else {
                // Forty nested closures exceed the existing parser depth ceiling;
                // recovery must still finish rather than retry each body.
                assert_eq!(
                    errors
                        .iter()
                        .filter(|error| error.message == "nesting too deep")
                        .count(),
                    1,
                    "{errors:?}"
                );
            }
        }
    }
}

#[test]
fn malformed_nullable_initializer_preserves_the_following_statement() {
    let source = "void f() { T? x = ; after(); }";
    let parsed = parse(source);
    assert!(!parsed.errors.is_empty());
    assert!(parsed.tree.is_lossless(source));
    let mut statements = body_stmts(parsed.tree.program(source));
    assert_eq!(statements.len(), 2);
    assert!(matches!(statements.next(), Some(Stmt::LocalVar(_))));
    assert!(matches!(statements.next(), Some(Stmt::Expr(_))));
}

#[test]
fn nullable_call_heads_remain_conditional_expressions() {
    for statement in [
        "cond ? run(onDone: () { T? x; }) : skip();",
        "cond ? run(() { T? x; }) : skip();",
        "cond ? run<T>(onDone: () { T? x; }) : skip();",
        "cond ? run() ?? {'key': 1} : skip();",
        "cond ? run() ? {'key': 1} : {'key': 2} : skip();",
    ] {
        let source = format!("void f() {{ {statement} after(); }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        let mut statements = body_stmts(parsed.tree.program(&source));
        assert_eq!(statements.len(), 2);
        assert!(matches!(
            statements.next(),
            Some(Stmt::Expr(statement)) if matches!(statement.expr(), Expr::Conditional(_))
        ));
    }
}

#[test]
fn nullable_function_return_type_does_not_scan_following_statements() {
    for declaration in [
        "T? getValue() { return null; }",
        "T? getValue<U>() { return null; }",
        "T? getValue() => cond ? a : b;",
    ] {
        let source = format!("void f() {{ {declaration} a ? b = 1 : c; }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert!(parsed.tree.is_lossless(&source));
        let mut statements = body_stmts(parsed.tree.program(&source));
        assert_eq!(statements.len(), 2);
        assert!(matches!(statements.next(), Some(Stmt::LocalFunc(_))));
        assert!(matches!(
            statements.next(),
            Some(Stmt::Expr(statement)) if matches!(statement.expr(), Expr::Conditional(_))
        ));
    }
}

#[test]
fn mismatched_closure_delimiters_preserve_the_conditional_head() {
    let source = "void f() { a ? b = () { g( } : c; }";
    let parsed = parse(source);
    assert!(!parsed.errors.is_empty());
    assert!(parsed.tree.is_lossless(source));
    assert!(matches!(
        body_stmts(parsed.tree.program(source)).next(),
        Some(Stmt::Expr(statement)) if matches!(statement.expr(), Expr::Conditional(_))
    ));
}
