//! Flags a local variable that is returned on the immediately following statement.
//!
//! Assigning a value to a variable only to return it on the next line adds an
//! intermediate name that carries no information; return the expression
//! directly. The rule fires when a single-declarator local variable declaration
//! is immediately followed by a `return` of that same variable. Every statement
//! list is checked, including closures, operators, switch cases, and finally blocks.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_stmt_list;

#[derive(Default)]
pub struct PreferImmediateReturn;

impl Rule for PreferImmediateReturn {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for_each_stmt_list(program, &mut |stmts, _| check_stmts(stmts, &mut diags, ctx));
        RulePreparation::Diagnostics(diags)
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
