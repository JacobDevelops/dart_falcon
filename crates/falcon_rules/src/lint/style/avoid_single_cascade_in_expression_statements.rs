//! Flags an expression statement that is a cascade with a single section.
//!
//! A cascade (`..`) exists to chain several operations on one receiver; with a
//! single section it buys nothing over an ordinary member access and only
//! obscures intent. Rewrite `foo..bar()` as `foo.bar()`. Cascades with two or
//! more sections are the idiomatic form and are not flagged.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

#[derive(Default)]
pub struct AvoidSingleCascadeInExpressionStatements;

impl Rule for AvoidSingleCascadeInExpressionStatements {
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
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Expr(e) = node
            && let Expr::Cascade { sections, span, .. } = &e.expr
            && sections.len() == 1
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-single-cascade-in-expression-statements",
                "Single cascade in an expression statement — use `.` instead of `..`",
                *span,
            ));
        }
        walk_stmt(self, node);
    }
}
