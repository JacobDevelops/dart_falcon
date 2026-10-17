use falcon_syntax::cst::{AstChildren, FunctionBody, Program, Stmt, TopLevelDecl};

pub fn body_stmts(prog: Program<'_>) -> AstChildren<'_, Stmt<'_>> {
    let func = match prog.declarations().first() {
        Some(TopLevelDecl::Function(f)) => f,
        other => panic!("expected function, got {other:?}"),
    };
    let block = match func.body() {
        Some(FunctionBody::Block(b)) => b,
        other => panic!("expected block body, got {other:?}"),
    };
    block.stmts()
}
