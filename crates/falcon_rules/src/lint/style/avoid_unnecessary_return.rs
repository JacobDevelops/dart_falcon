//! Flags bare returns at the end of a function or its final if/block branches.
//!
//! Falling through already exits the function. Only paths through final block
//! statements and if branches are followed; loops, switch, try/finally, and
//! value-returning statements are deliberately excluded. Nested functions have
//! their own boundary, so their returns cannot escape into an enclosing body.
//! Identical if/else branches are left to no-equal-then-else.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryReturn;

impl Rule for AvoidUnnecessaryReturn {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

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

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Return(ret) = node
            && ret.value().is_none()
            && at_function_end(node)
            && !node.syntax().ancestors()
                .take_while(|ancestor| Stmt::cast(*ancestor).is_some())
                .filter_map(IfStmt::cast)
                .any(|branch| branch.else_branch().is_some_and(|otherwise| {
                    let normalize = crate::lint::suspicious::no_equal_then_else::normalize;
                    normalize(self.ctx, branch.then_branch().span())
                        .eq(normalize(self.ctx, otherwise.span()))
                }))
        {
            self.diags.push(self.ctx.diagnostic(
                "This trailing return is unnecessary; let the function finish naturally.",
                ret.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
