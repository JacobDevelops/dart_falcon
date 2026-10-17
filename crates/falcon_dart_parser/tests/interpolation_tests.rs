//! String-interpolation parsing: the `StringLitNode::interpolations` are real
//! parsed expressions carrying absolute source spans.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::*;

#[path = "common/string_lit.rs"]
mod common_string;
use common_string::string_lit;

/// The source text under an interpolation's span.
fn span_text<'a>(src: &'a str, interp: StringInterpolation<'_>) -> &'a str {
    &src[interp.span().start..interp.span().end]
}

#[test]
fn simple_identifier_interpolation() {
    let src = format!("void f() {{ var x = {}; }}", "'$a'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 1);
    assert_eq!(span_text(&src, node.interpolations().next().unwrap()), "a");
    assert!(
        matches!(node.interpolations().next().unwrap().expr(), Expr::Ident(id) if id.name() == "a")
    );
}

#[test]
fn this_interpolation_is_a_this_expression() {
    let src = format!("void f() {{ var x = {}; }}", "'$this $thisValue ${this}'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 3);
    for index in [0, 2] {
        let interpolation = node.interpolations().nth(index).unwrap();
        assert!(matches!(interpolation.expr(), Expr::This(_)));
        assert_eq!(span_text(&src, interpolation), "this");
        assert_eq!(interpolation.expr().span(), interpolation.span());
    }
    assert!(
        matches!(node.interpolations().nth(1).unwrap().expr(), Expr::Ident(id) if id.name() == "thisValue")
    );
}

#[test]
fn braced_binary_interpolation() {
    let src = format!("void f() {{ var x = {}; }}", "'${a + b}'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 1);
    assert_eq!(
        span_text(&src, node.interpolations().next().unwrap()),
        "a + b"
    );
    assert!(matches!(
        node.interpolations().next().unwrap().expr(),
        Expr::Binary(_)
    ));
}

#[test]
fn braced_interpolation_inherits_async_body_context() {
    for body in ["async", "async*"] {
        for expression in ["await f()", "await value", "'${await f()}'"] {
            let src = format!("void f() {body} {{ var x = '${{{expression}}}'; }}");
            let parsed = parse(&src);
            assert!(parsed.errors.is_empty(), "{src}: {:?}", parsed.errors);
            let node = string_lit(parsed.tree.program(&src));
            let interpolation = node.interpolations().next().unwrap();
            if expression.starts_with("await") {
                assert!(matches!(interpolation.expr(), Expr::Await(_)), "{src}");
            } else {
                let Expr::StringLit(nested) = interpolation.expr() else {
                    panic!("{src}");
                };
                assert!(matches!(
                    nested.interpolations().next().unwrap().expr(),
                    Expr::Await(_)
                ));
            }
        }
    }
}

#[test]
fn braced_interpolation_keeps_sync_body_context() {
    for body in ["", "sync*"] {
        let src = format!("void f() {body} {{ var x = '${{await f()}}'; }}");
        let parsed = parse(&src);
        let node = string_lit(parsed.tree.program(&src));
        assert_eq!(node.interpolations().count(), 0, "{src}");
    }
}

#[test]
fn interpolation_closures_have_their_own_body_context() {
    for source in [
        "void f() async { var x = '${() { var await = 1; return await; }}'; }",
        "void f() async* { yield '${() { yield(1); }}'; }",
        "class C { var g; C() : g = '${() {}}'; }",
        "class C { var g; C() : g = c ? '${() {}}' : null; }",
    ] {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        let nodes: Vec<_> = parsed.tree.root(source).descendants().collect();
        assert!(
            nodes.iter().any(|node| node.kind() == NodeKind::FuncExpr),
            "{source}"
        );
        assert!(
            !nodes.iter().any(|node| node.kind() == NodeKind::AwaitExpr),
            "{source}"
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.kind() == NodeKind::YieldStmt)
                .count(),
            usize::from(source.contains("async*")),
            "{source}"
        );
    }
}

#[test]
fn generator_yield_interpolation_inherits_its_body_marker() {
    for body in ["sync*", "async*"] {
        let source = format!("void f() {body} {{ yield '${{await g()}}'; }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
        assert_eq!(
            parsed
                .tree
                .root(&source)
                .descendants()
                .filter(|node| node.kind() == NodeKind::AwaitExpr)
                .count(),
            usize::from(body == "async*")
        );
    }
}

#[test]
fn braced_interpolation_retains_interior_trivia_but_expression_stays_precise() {
    let src = format!(
        "void f() {{ var x = {}; }}",
        "'${ /* before */ a + b /* after */ }'"
    );
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    let interpolation = node.interpolations().next().unwrap();
    assert_eq!(
        span_text(&src, interpolation),
        " /* before */ a + b /* after */ "
    );
    let span = interpolation.expr().span();
    assert_eq!(&src[span.start..span.end], "a + b");
}

#[test]
fn interpolation_amidst_text() {
    let src = format!("void f() {{ var x = {}; }}", "'x ${f(y)} z'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 1);
    assert_eq!(
        span_text(&src, node.interpolations().next().unwrap()),
        "f(y)"
    );
    assert!(matches!(
        node.interpolations().next().unwrap().expr(),
        Expr::Call(_)
    ));
}

#[test]
fn braced_interpolation_span_preserves_full_interior() {
    for interior in [" a ", " /* leading */ a /* trailing */ ", "\n a \n"] {
        let src = format!("void f() {{ var x = '''${{{interior}}}'''; }}");
        let parsed = parse(&src);
        assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
        let node = string_lit(parsed.tree.program(&src));
        assert_eq!(node.interpolations().count(), 1);
        let interpolation = node.interpolations().next().unwrap();
        assert_eq!(span_text(&src, interpolation), interior);
        let expr_span = interpolation.expr().span();
        assert_eq!(&src[expr_span.start..expr_span.end], "a");
        assert!(matches!(&interpolation.expr(), Expr::Ident(id) if id.name() == "a"));
    }
}

#[test]
fn nested_interpolation() {
    let src = format!("void f() {{ var x = {}; }}", "'${'${a}'}'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 1);
    assert_eq!(
        span_text(&src, node.interpolations().next().unwrap()),
        "'${a}'"
    );
    let Expr::StringLit(inner) = node.interpolations().next().unwrap().expr() else {
        panic!("expected a nested string literal");
    };
    assert_eq!(inner.interpolations().count(), 1);
    assert_eq!(span_text(&src, inner.interpolations().next().unwrap()), "a");
    assert!(
        matches!(inner.interpolations().next().unwrap().expr(), Expr::Ident(id) if id.name() == "a")
    );
}

#[test]
fn triple_quoted_interpolation() {
    let src = format!("void f() {{ var x = {}; }}", "'''hello $name'''");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 1);
    assert_eq!(
        span_text(&src, node.interpolations().next().unwrap()),
        "name"
    );
    assert!(
        matches!(node.interpolations().next().unwrap().expr(), Expr::Ident(id) if id.name() == "name")
    );
}

#[test]
fn raw_string_has_no_interpolations() {
    let src = format!("void f() {{ var x = {}; }}", "r'$name'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert!(node.interpolations().next().is_none());
}

#[test]
fn adjacent_strings_concatenate_interpolations() {
    let src = format!("void f() {{ var x = {}; }}", "'$a' '$b'");
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let node = string_lit(parsed.tree.program(&src));
    assert_eq!(node.interpolations().count(), 2);
    assert_eq!(span_text(&src, node.interpolations().next().unwrap()), "a");
    assert_eq!(span_text(&src, node.interpolations().nth(1).unwrap()), "b");
}

#[test]
fn malformed_inner_expression_is_dropped_but_error_propagates() {
    // `${` with an unparseable body records no interpolation, but its parse
    // diagnostic reaches the enclosing program with an absolute source offset.
    let source = "void f() { var x = '${a +}'; }";
    let parsed = parse(source);
    let errors = &parsed.errors;
    let program = parsed.tree.program(source);
    assert!(
        !errors.is_empty(),
        "expected a propagated interpolation error"
    );

    let TopLevelDecl::Function(f) = program.declarations().next().unwrap() else {
        panic!("expected a top-level function");
    };
    let Some(FunctionBody::Block(block)) = f.body() else {
        panic!("expected a block body");
    };
    let Stmt::LocalVar(lv) = block.stmts().next().unwrap() else {
        panic!("expected a local var");
    };
    let Some(Expr::StringLit(node)) = lv.declarators().next().unwrap().initializer() else {
        panic!("expected a string literal initializer");
    };
    assert!(
        node.interpolations().next().is_none(),
        "the malformed interpolation is omitted from the AST"
    );

    // The diagnostic's offset is absolute and falls inside the `${...}` body.
    let body_start = source.find("${").unwrap() + 2;
    let body_end = source.find('}').unwrap();
    assert!(
        errors
            .iter()
            .any(|e| e.offset >= body_start && e.offset <= body_end),
        "diagnostic offsets {:?} should fall within the interpolation body {body_start}..={body_end}",
        errors.iter().map(|e| e.offset).collect::<Vec<_>>(),
    );
}
