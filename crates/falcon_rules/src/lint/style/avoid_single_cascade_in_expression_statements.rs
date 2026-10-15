//! Flags an expression statement that is a cascade with a single section.
//!
//! A cascade (`..`) exists to chain several operations on one receiver; with a
//! single section it buys nothing over an ordinary member access and only
//! obscures intent. Rewrite `foo..bar()` as `foo.bar()`. Cascades with two or
//! more sections are the idiomatic form and are not flagged.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_stmt_in_program;

#[derive(Default)]
pub struct AvoidSingleCascadeInExpressionStatements;

impl Rule for AvoidSingleCascadeInExpressionStatements {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_stmt_in_program(program, |node| {
            if let Stmt::Expr(e) = node
                && let Expr::Cascade { sections, span, .. } = e.expr.unparenthesized()
                && sections.len() == 1
            {
                diags.push(ctx.diagnostic(
                    "avoid-single-cascade-in-expression-statements",
                    "Single cascade in an expression statement — use `.` instead of `..`",
                    *span,
                ));
            }
        });
        diags
    }
}
