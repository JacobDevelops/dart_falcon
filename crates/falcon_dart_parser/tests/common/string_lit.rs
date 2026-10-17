use falcon_syntax::cst::{Expr, FunctionBody, Program, Stmt, StringLitNode, TopLevelDecl};

pub fn string_lit(program: Program<'_>) -> StringLitNode<'_> {
    let TopLevelDecl::Function(f) = program.declarations().first().unwrap() else {
        panic!("expected a top-level function");
    };
    let Some(FunctionBody::Block(block)) = f.body() else {
        panic!("expected a block body");
    };
    let Stmt::LocalVar(lv) = block.stmts().first().unwrap() else {
        panic!("expected a local var");
    };
    let Some(Expr::StringLit(node)) = lv.declarators().first().unwrap().initializer() else {
        panic!("expected a string literal initializer");
    };
    node
}
