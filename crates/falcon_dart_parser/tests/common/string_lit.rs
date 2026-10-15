use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::{Expr, FunctionBody, Stmt, StringLitNode, TopLevelDecl};

pub fn string_lit(expr_src: &str) -> (StringLitNode, String) {
    let src = format!("void f() {{ var x = {expr_src}; }}");
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(&src);
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
