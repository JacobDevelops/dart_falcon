//! String-interpolation parsing: the `StringLitNode::interpolations` are real
//! parsed expressions carrying absolute source spans.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;

#[path = "common/string_lit.rs"]
mod common_string;
use common_string::string_lit;

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
fn this_interpolation_is_a_this_expression() {
    let (node, src) = string_lit("'$this $thisValue ${this}'");
    assert_eq!(node.interpolations.len(), 3);
    for index in [0, 2] {
        let interpolation = &node.interpolations[index];
        assert!(matches!(interpolation.expr, Expr::This { .. }));
        assert_eq!(span_text(&src, interpolation), "this");
        assert_eq!(*interpolation.expr.span(), interpolation.span);
    }
    assert!(matches!(&node.interpolations[1].expr, Expr::Ident(id) if id.name == "thisValue"));
}

#[test]
fn braced_binary_interpolation() {
    let (node, src) = string_lit("'${a + b}'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "a + b");
    assert!(matches!(node.interpolations[0].expr, Expr::Binary { .. }));
}

#[test]
fn braced_interpolation_retains_interior_trivia_but_expression_stays_precise() {
    let (node, src) = string_lit("'${ /* before */ a + b /* after */ }'");
    let interpolation = &node.interpolations[0];
    assert_eq!(
        span_text(&src, interpolation),
        " /* before */ a + b /* after */ "
    );
    let span = interpolation.expr.span();
    assert_eq!(&src[span.start..span.end], "a + b");
}

#[test]
fn interpolation_amidst_text() {
    let (node, src) = string_lit("'x ${f(y)} z'");
    assert_eq!(node.interpolations.len(), 1);
    assert_eq!(span_text(&src, &node.interpolations[0]), "f(y)");
    assert!(matches!(node.interpolations[0].expr, Expr::Call { .. }));
}

#[test]
fn braced_interpolation_span_preserves_full_interior() {
    for interior in [" a ", " /* leading */ a /* trailing */ ", "\n a \n"] {
        let (node, src) = string_lit(&format!("'''${{{interior}}}'''"));
        assert_eq!(node.interpolations.len(), 1);
        let interpolation = &node.interpolations[0];
        assert_eq!(span_text(&src, interpolation), interior);
        let expr_span = interpolation.expr.span();
        assert_eq!(&src[expr_span.start..expr_span.end], "a");
        assert!(matches!(&interpolation.expr, Expr::Ident(id) if id.name == "a"));
    }
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
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(src);
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
