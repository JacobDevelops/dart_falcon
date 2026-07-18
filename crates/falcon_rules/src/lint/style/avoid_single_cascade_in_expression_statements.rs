//! Flags an expression statement that is a cascade with a single section.
//!
//! A cascade (`..`) exists to chain several operations on one receiver; with a
//! single section it buys nothing over an ordinary member access and only
//! obscures intent. Rewrite `foo..bar()` as `foo.bar()`. Cascades with two or
//! more sections are the idiomatic form and are not flagged.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

pub struct AvoidSingleCascadeInExpressionStatements;

impl Rule for AvoidSingleCascadeInExpressionStatements {
    fn name(&self) -> &'static str {
        "avoid-single-cascade-in-expression-statements"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Expr(e) = node
            && let Expr::Cascade { sections, span, .. } = &e.expr
            && sections.len() == 1
        {
            self.diags.push(Diagnostic::new(
                "avoid-single-cascade-in-expression-statements",
                Severity::Warning,
                "Single cascade in an expression statement — use `.` instead of `..`",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_stmt(self, node);
    }
}
