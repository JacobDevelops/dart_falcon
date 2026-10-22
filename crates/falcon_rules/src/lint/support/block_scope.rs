use falcon_syntax::cst::*;

/// Require the same direct block: no branch-dominance or closure assumptions.
pub(crate) fn same_block(declaration: SyntaxNode<'_>, mut usage: SyntaxNode<'_>) -> bool {
    let Some(block) = declaration
        .parent()
        .filter(|node| node.kind() == NodeKind::Block)
    else {
        return false;
    };
    while let Some(parent) = usage.parent() {
        if let Some(statement) = Stmt::cast(parent) {
            return matches!(
                statement,
                Stmt::LocalVar(_) | Stmt::Expr(_) | Stmt::Return(_) | Stmt::Throw(_)
            ) && parent
                .parent()
                .is_some_and(|scope| scope.id() == block.id())
                && declaration.span().end <= parent.span().start;
        }
        if matches!(parent.kind(), NodeKind::FuncExpr | NodeKind::SwitchExpr) {
            return false;
        }
        usage = parent;
    }
    false
}
