use falcon_syntax::ast::{Expr, FunctionBody, Program, Stmt, TopLevelDecl};

pub fn first_stmt_expr(prog: &Program) -> Expr {
    let fun = match &prog.declarations[0] {
        TopLevelDecl::Function(fun) => fun,
        other => panic!("expected function, got {other:?}"),
    };
    let block = match &fun.body {
        Some(FunctionBody::Block(block)) => block,
        other => panic!("expected block body, got {other:?}"),
    };
    let stmt = match &block.stmts[0] {
        Stmt::Expr(stmt) => stmt,
        other => panic!("expected expr stmt, got {other:?}"),
    };
    stmt.expr.clone()
}
