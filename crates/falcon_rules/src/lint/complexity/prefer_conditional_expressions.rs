//! Flags an `if`/`else` whose branches each reduce to a single return or a
//! single assignment to the same target.
//!
//! When both branches differ only in the value they return or assign, a
//! conditional expression — `return c ? a : b;` or `x = c ? a : b;` — says the
//! same thing in one line. The rule reports only the outermost `if` of a chain
//! (whose parent is not an `if` and whose `else` is not an `else if`) where both
//! branches reduce to a single return, or to a single assignment to the same
//! variable. Local functions, closures, and labeled statements are included.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferConditionalExpressions;

impl Rule for PreferConditionalExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(ConditionalExpressions {
            diags: Vec::new(),
            ctx,
            parent_is_if: false,
            parents: Vec::new(),
        }))
    }
}

fn unwrap_single(mut stmt: Stmt<'_>) -> Stmt<'_> {
    while let Stmt::Block(block) = stmt {
        if block.stmts().len() != 1 {
            break;
        }
        stmt = block.stmts().first().unwrap();
    }
    stmt
}

fn is_return(stmt: Stmt<'_>) -> bool {
    matches!(unwrap_single(stmt), Stmt::Return(_))
}

/// The simple-identifier target of a single assignment statement, if any.
fn assign_target_name(stmt: Stmt<'_>) -> Option<&str> {
    if let Stmt::Expr(e) = unwrap_single(stmt)
        && let Expr::Assign(assign) = e.expr().unparenthesized()
        && let Expr::Ident(id) = assign.target().unparenthesized()
    {
        return Some(id.name());
    }
    None
}

struct ConditionalExpressions<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    parent_is_if: bool,
    parents: Vec<bool>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for ConditionalExpressions<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(n) = node
            && let Some(else_branch) = n.else_branch()
            && !self.parent_is_if
            && !matches!(else_branch, Stmt::If(_))
        {
            let both_return = is_return(n.then_branch()) && is_return(else_branch);
            let both_assign = matches!(
                (assign_target_name(n.then_branch()), assign_target_name(else_branch)),
                (Some(a), Some(b)) if a == b
            );
            if both_return || both_assign {
                self.diags.push(self.ctx.diagnostic(
                    "Prefer a conditional expression over an if/else with a single statement in each branch",
                    n.span(),
                ));
            }
        }
        self.parents.push(self.parent_is_if);
        self.parent_is_if = matches!(node, Stmt::If(_));
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        self.parent_is_if = self.parents.pop().unwrap();
    }

    fn visit_expr(&mut self, _node: Expr<'a>) -> StructuralTraversal {
        self.parents.push(self.parent_is_if);
        self.parent_is_if = false;
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.parent_is_if = self.parents.pop().unwrap();
    }
}
}
