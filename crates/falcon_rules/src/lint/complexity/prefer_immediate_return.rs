//! Flags a local variable that is returned on the immediately following statement.
//!
//! Assigning a value to a variable only to return it on the next line adds an
//! intermediate name that carries no information; return the expression
//! directly. The rule fires when a single-declarator local variable declaration
//! is immediately followed by a `return` of that same variable. Every statement
//! list is checked, including closures, operators, switch cases, and finally blocks.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{body_list, member_list, stmt_lists};

#[derive(Default)]
pub struct PreferImmediateReturn;

impl Rule for PreferImmediateReturn {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        body_list(node.body.as_ref(), &mut |stmts, _| check_stmts(stmts, &mut self.diags, self.ctx));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        stmt_lists(node, &mut |stmts, _| check_stmts(stmts, &mut self.diags, self.ctx));
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        member_list(node, &mut |stmts, _| check_stmts(stmts, &mut self.diags, self.ctx));
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::FuncExpr { body, .. } = node {
            body_list(Some(body), &mut |stmts, _| check_stmts(stmts, &mut self.diags, self.ctx));
        }
        StructuralTraversal::Continue
    }
}
}

fn check_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Check pairs: LocalVar immediately followed by Return of that var
    for i in 0..stmts.len().saturating_sub(1) {
        if let Stmt::LocalVar(local) = &stmts[i]
            && local.declarators.len() == 1
        {
            let var_name = &local.declarators[0].name.name;
            if let Stmt::Return(ret) = &stmts[i + 1]
                && let Some(Expr::Ident(ident)) = ret.value.as_ref().map(Expr::unparenthesized)
                && &ident.name == var_name
            {
                diags.push(ctx.diagnostic(
                    "prefer-immediate-return",
                    "Prefer returning the value directly instead of assigning to a variable first",
                    local.full_range(),
                ));
            }
        }
    }
}
