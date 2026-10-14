//! Flags `if (x == null) { x = y; }` with no else branch.
//!
//! A null-guarded assignment like this is exactly what the `??=` compound
//! operator expresses: `x ??= y`. The shorter form removes a branch and states
//! the intent — assign only when currently null — directly. The rule matches an
//! `if` with no `else` whose condition is an `== null` check and whose single
//! then-statement assigns to that same operand.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct PreferConditionalAssignment;

impl Rule for PreferConditionalAssignment {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
    source: &'s str,
}

/// The non-null operand of a `E == null` comparison.
fn eq_null_operand(cond: &Expr) -> Option<&Expr> {
    let Expr::Binary {
        op: BinaryOp::EqEq,
        left,
        right,
        ..
    } = cond
    else {
        return None;
    };
    match (left.as_ref(), right.as_ref()) {
        (Expr::NullLit { .. }, other) | (other, Expr::NullLit { .. }) => Some(other),
        _ => None,
    }
}

/// The single statement of a `then` branch, unwrapping a one-statement block.
fn single_stmt(stmt: &Stmt) -> Option<&Stmt> {
    match stmt {
        Stmt::Block(b) if b.stmts.len() == 1 => Some(&b.stmts[0]),
        Stmt::Block(_) => None,
        other => Some(other),
    }
}

use crate::lint::expressions::text;

impl Visitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::If(if_stmt) = node
            && if_stmt.else_branch.is_none()
            && let IfCondition::Expr(cond) = &if_stmt.condition
            && let Some(checked) = eq_null_operand(cond)
            && let Some(Stmt::Expr(es)) = single_stmt(&if_stmt.then_branch)
            && let Expr::Assign {
                target,
                op: AssignOp::Eq,
                ..
            } = &es.expr
            && text(target, self.source) == text(checked, self.source)
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-conditional-assignment",
                "Prefer using a conditional assignment '??='.",
                if_stmt.span,
            ));
        }
        walk_stmt(self, node);
    }
}
