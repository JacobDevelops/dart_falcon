//! Flags `if (x == null) { x = y; }` with no else branch.
//!
//! A null-guarded assignment like this is exactly what the `??=` compound
//! operator expresses: `x ??= y`. The shorter form removes a branch and states
//! the intent — assign only when currently null — directly. The rule matches an
//! `if` with no `else` whose condition is an `== null` check and whose single
//! then-statement assigns to the same canonical identifier/property chain.
//! Indexed targets are not identifiers; inherited accessor halves must belong
//! to the same declaring variable.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::complexity::join_return_with_assignment::same_element;
use crate::lint::expressions::null_check;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferConditionalAssignment;

impl Rule for PreferConditionalAssignment {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

/// The single statement of a `then` branch, unwrapping a one-statement block.
fn single_stmt(stmt: Stmt<'_>) -> Option<Stmt<'_>> {
    match stmt {
        Stmt::Block(b) if b.stmts().len() == 1 => single_stmt(b.stmts().first()?),
        Stmt::Block(_) => None,
        other => Some(other),
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::If(if_stmt) = node
            && if_stmt.else_branch().is_none()
            && let IfCondition::Expr(cond) = if_stmt.condition()
            && let Some((checked, true)) = null_check(cond)
            && let Some(Stmt::Expr(es)) = single_stmt(if_stmt.then_branch())
            && let Expr::Assign(assign) = es.expr()
            && same_element(assign.target(), true, checked, false, state)
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-conditional-assignment",
                "Prefer using a conditional assignment '??='.",
                if_stmt.span(),
            ));
        }
    }
}
