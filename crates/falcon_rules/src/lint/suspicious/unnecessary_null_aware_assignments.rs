//! Flags `x ??= null`, which can never have an effect.
//!
//! The `??=` operator assigns only when the target is currently null, so
//! assigning `null` is a guaranteed no-op: a null target stays null, and
//! otherwise nothing happens. Such a statement is dead code that usually points
//! to a mistaken right-hand side — the author likely meant a real default value.
//! Remove the statement, or supply the value that was intended.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UnnecessaryNullAwareAssignments;

impl Rule for UnnecessaryNullAwareAssignments {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Assign {
            op: AssignOp::NullCoalesceEq,
            value,
            span,
            ..
        } = node
            && matches!(value.unparenthesized(), Expr::NullLit { .. })
        {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-null-aware-assignments",
                "Unnecessary null-aware assignment to null.",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}
