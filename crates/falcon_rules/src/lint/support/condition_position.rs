use falcon_syntax::cst::*;

pub(crate) fn is_in_condition(mut child: SyntaxNode<'_>) -> bool {
    while let Some(parent) = child.parent() {
        match parent.kind() {
            NodeKind::ExprCondition | NodeKind::ForCondition | NodeKind::ExprGuard => return true,
            NodeKind::FuncExpr => return false,
            _ => {}
        }
        if let Some(label) = CasePatternLabel::cast(parent)
            && label.guard().is_some_and(|guard| guard.syntax() == child)
        {
            return true;
        }
        if let Some(statement) = Stmt::cast(parent) {
            return match statement {
                Stmt::If(node) => node.condition().syntax() == child,
                Stmt::While(node) => node.condition().syntax() == child,
                Stmt::DoWhile(node) => node.condition().syntax() == child,
                _ => false,
            };
        }
        if let Some(Expr::Conditional(node)) = Expr::cast(parent)
            && node.condition().syntax() == child
        {
            return true;
        }
        child = parent;
    }
    false
}
