//! Flags an `if`/`else` whose branches each reduce to a single return or a
//! single assignment to the same target.
//!
//! When both branches differ only in the value they return or assign, a
//! conditional expression — `return c ? a : b;` or `x = c ? a : b;` — says the
//! same thing in one line. The rule reports only the outermost `if` of a chain
//! (whose parent is not an `if` and whose `else` is not an `else if`) where both
//! branches reduce to a single return, or to a single assignment to the same
//! variable. Local functions, closures, and labeled statements are included.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct PreferConditionalExpressions;

impl Rule for PreferConditionalExpressions {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        ConditionalExpressions {
            diags: &mut diags,
            ctx,
            parent_is_if: false,
        }
        .visit_program(program);
        diags
    }
}

fn unwrap_single(mut stmt: &Stmt) -> &Stmt {
    while let Stmt::Block(block) = stmt {
        if block.stmts.len() != 1 {
            break;
        }
        stmt = &block.stmts[0];
    }
    stmt
}

fn is_return(stmt: &Stmt) -> bool {
    matches!(unwrap_single(stmt), Stmt::Return(_))
}

/// The simple-identifier target of a single assignment statement, if any.
fn assign_target_name(stmt: &Stmt) -> Option<&str> {
    if let Stmt::Expr(e) = unwrap_single(stmt)
        && let Expr::Assign { target, .. } = e.expr.unparenthesized()
        && let Expr::Ident(id) = target.unparenthesized()
    {
        return Some(&id.name);
    }
    None
}

struct ConditionalExpressions<'a, 'ctx> {
    diags: &'a mut Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    parent_is_if: bool,
}

impl Visitor for ConditionalExpressions<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::If(n) = node
            && let Some(else_branch) = &n.else_branch
            && !self.parent_is_if
            && !matches!(else_branch.as_ref(), Stmt::If(_))
        {
            let both_return = is_return(&n.then_branch) && is_return(else_branch);
            let both_assign = matches!(
                (assign_target_name(&n.then_branch), assign_target_name(else_branch)),
                (Some(a), Some(b)) if a == b
            );
            if both_return || both_assign {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-conditional-expressions",
                    "Prefer a conditional expression over an if/else with a single statement in each branch",
                    n.span,
                ));
            }
        }
        let parent = self.parent_is_if;
        self.parent_is_if = matches!(node, Stmt::If(_));
        walk_stmt(self, node);
        self.parent_is_if = parent;
    }

    fn visit_expr(&mut self, node: &Expr) {
        let parent = self.parent_is_if;
        self.parent_is_if = false;
        walk_expr(self, node);
        self.parent_is_if = parent;
    }
}
