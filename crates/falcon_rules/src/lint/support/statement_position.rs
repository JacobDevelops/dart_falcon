use falcon_syntax::cst::*;

/// Whether falling through this statement also leaves its function body.
pub(crate) fn at_function_end(statement: Stmt<'_>) -> bool {
    let mut current = statement.syntax();
    while let Some(parent) = current.parent() {
        if let Some(block) = Block::cast(parent) {
            if block.stmts().last().map(|last| last.syntax().id()) != Some(current.id()) {
                return false;
            }
        } else if IfStmt::cast(parent).is_none() {
            return FunctionDecl::cast(parent).is_some()
                || LocalFuncDecl::cast(parent).is_some()
                || FuncExpr::cast(parent).is_some()
                || matches!(
                    ClassMember::cast(parent),
                    Some(
                        ClassMember::Method(_)
                            | ClassMember::Getter(_)
                            | ClassMember::Setter(_)
                            | ClassMember::Operator(_)
                            | ClassMember::Constructor(_)
                            | ClassMember::PrimaryConstructorBody(_)
                    )
                );
        }
        current = parent;
    }
    false
}
