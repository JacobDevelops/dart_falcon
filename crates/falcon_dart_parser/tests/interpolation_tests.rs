//! String-interpolation parsing: the `StringLitNode::interpolations` are real
//! parsed expressions carrying absolute source spans.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

/// Parse `void f() { var x = <expr>; }` and return the initializer string
/// literal together with the full wrapped source (so spans can be sliced).
fn string_lit(expr_src: &str) -> (StringLitNode, String) {
    let src = format!("void f() {{ var x = {expr_src}; }}");
    let (program, errors) = parse(&src);
    assert!(
        errors.is_empty(),
        "parse errors for `{expr_src}`: {errors:?}"
    );
    let TopLevelDecl::Function(f) = &program.declarations[0] else {
        panic!("expected a top-level function");
    };
    let Some(FunctionBody::Block(block)) = &f.body else {
        panic!("expected a block body");
    };
    let Stmt::LocalVar(lv) = &block.stmts[0] else {
        panic!("expected a local var");
    };
    let Some(Expr::StringLit(node)) = &lv.declarators[0].initializer else {
        panic!("expected a string literal initializer");
    };
    (node.clone(), src)
}

/// The source text under an interpolation's span.
fn span_text<'a>(src: &'a str, interp: &StringInterpolation) -> &'a str {
    &src[interp.span.start..interp.span.end]
}

#[test]
fn simple_identifier_interpolation() {
    let (node, src) = string_lit("'$a'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "a");
    assert!(matches!(&node.interpolations[0].expr, Expr::Ident(id) if id.name == "a"));
}

#[test]
fn braced_binary_interpolation() {
    let (node, src) = string_lit("'${a + b}'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "a + b");
    assert!(matches!(node.interpolations[0].expr, Expr::Binary { .. }));
}

#[test]
fn interpolation_amidst_text() {
    let (node, src) = string_lit("'x ${f(y)} z'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "f(y)");
    assert!(matches!(node.interpolations[0].expr, Expr::Call { .. }));
}

#[test]
fn braced_expression_span_is_trimmed_to_expression() {
    // Interior whitespace inside the braces is not part of the expression span.
    let (node, src) = string_lit("'${ a }'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "a");
}

#[test]
fn nested_interpolation() {
    let (node, src) = string_lit("'${'${a}'}'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "'${a}'");
    let Expr::StringLit(inner) = &node.interpolations[0].expr else {
        panic!("expected a nested string literal");
    };
    assert_eq!(inner.interpolations.len(), 1);
    assert_eq!(span_text(&src, &inner.interpolations[0]), "a");
    assert!(matches!(&inner.interpolations[0].expr, Expr::Ident(id) if id.name == "a"));
}

#[test]
fn triple_quoted_interpolation() {
    let (node, src) = string_lit("'''hello $name'''");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "name");
    assert!(matches!(&node.interpolations[0].expr, Expr::Ident(id) if id.name == "name"));
}

#[test]
fn raw_string_has_no_interpolations() {
    let (node, _src) = string_lit("r'$name'");
    assert!(node.interpolations.is_empty());
}

#[test]
fn adjacent_strings_concatenate_interpolations() {
    let (node, src) = string_lit("'$a' '$b'");
    assert_eq!(node.interpolations.len(), 2);
    assert_eq!(span_text(&src, &node.interpolations[0]), "a");
    assert_eq!(span_text(&src, &node.interpolations[1]), "b");
}

#[test]
fn malformed_inner_expression_is_dropped_but_error_propagates() {
    // `${` with an unparseable body records no interpolation, but its parse
    // diagnostic reaches the enclosing program with an absolute source offset.
    let src = "void f() { var x = '${a +}'; }";
    let (program, errors) = parse(src);
    assert!(
        !errors.is_empty(),
        "expected a propagated interpolation error"
    );

    let TopLevelDecl::Function(f) = &program.declarations[0] else {
        panic!("expected a top-level function");
    };
    let Some(FunctionBody::Block(block)) = &f.body else {
        panic!("expected a block body");
    };
    let Stmt::LocalVar(lv) = &block.stmts[0] else {
        panic!("expected a local var");
    };
    let Some(Expr::StringLit(node)) = &lv.declarators[0].initializer else {
        panic!("expected a string literal initializer");
    };
    assert!(
        node.interpolations.is_empty(),
        "the malformed interpolation is omitted from the AST"
    );

    // The diagnostic's offset is absolute and falls inside the `${...}` body.
    let body_start = src.find("${").unwrap() + 2;
    let body_end = src.find('}').unwrap();
    assert!(
        errors
            .iter()
            .any(|e| e.offset >= body_start && e.offset <= body_end),
        "diagnostic offsets {:?} should fall within the interpolation body {body_start}..={body_end}",
        errors.iter().map(|e| e.offset).collect::<Vec<_>>(),
    );
}
