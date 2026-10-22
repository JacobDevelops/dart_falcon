use falcon_syntax::cst::*;

pub(crate) fn field_name(expr: Expr<'_>) -> Option<&str> {
    match expr.unparenthesized() {
        Expr::Ident(id) => Some(id.name()),
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
            Some(field.field().name())
        }
        _ => None,
    }
}

pub(crate) fn getter_returns_field(getter: GetterDecl<'_>) -> Option<&str> {
    match getter.body()? {
        FunctionBody::Arrow(body) => field_name(body.expr()),
        FunctionBody::Block(block) => {
            let mut stmts = block.stmts();
            let stmt = stmts.next()?;
            if stmts.next().is_some() {
                return None;
            }
            match stmt {
                Stmt::Return(ret) => field_name(ret.value()?),
                _ => None,
            }
        }
        _ => None,
    }
}
