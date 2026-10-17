use falcon_syntax::cst::{Expr, FunctionBody, Program, Stmt, TopLevelDecl};

pub fn first_stmt_expr(prog: Program<'_>) -> Expr<'_> {
    let fun = match prog.declarations().first().unwrap() {
        TopLevelDecl::Function(fun) => fun,
        other => panic!("expected function, got {other:?}"),
    };
    let block = match fun.body() {
        Some(FunctionBody::Block(block)) => block,
        other => panic!("expected block body, got {other:?}"),
    };
    let stmt = match block.stmts().first().unwrap() {
        Stmt::Expr(stmt) => stmt,
        other => panic!("expected expr stmt, got {other:?}"),
    };
    stmt.expr()
}
