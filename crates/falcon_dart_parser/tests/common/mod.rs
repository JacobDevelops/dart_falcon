use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::{FunctionBody, Stmt, TopLevelDecl};

pub fn parse_body(src: &str, marker: &str) -> (Vec<Stmt>, usize) {
    let separator = if marker.is_empty() { "" } else { " " };
    let falcon_dart_parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(&format!("void f() {marker}{separator}{{ {src} }}"));
    let func = match prog.declarations.first() {
        Some(TopLevelDecl::Function(f)) => f,
        other => panic!("expected function, got {other:?}\nerrors: {errors:?}"),
    };
    let block = match func.body.as_ref() {
        Some(FunctionBody::Block(b)) => b,
        other => panic!("expected block body, got {other:?}"),
    };
    (block.stmts.clone(), errors.len())
}
