//! Flags `x ??= null`, which can never have an effect.
//!
//! The `??=` operator assigns only when the target is currently null, so
//! assigning `null` is a guaranteed no-op: a null target stays null, and
//! otherwise nothing happens. Such a statement is dead code that usually points
//! to a mistaken right-hand side — the author likely meant a real default value.
//! Remove the statement, or supply the value that was intended.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct UnnecessaryNullAwareAssignments;

impl Rule for UnnecessaryNullAwareAssignments {
    fn name(&self) -> &'static str {
        "unnecessary-null-aware-assignments"
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
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Assign {
            op: AssignOp::NullCoalesceEq,
            value,
            span,
            ..
        } = node
            && matches!(value.as_ref(), Expr::NullLit { .. })
        {
            self.diags.push(Diagnostic::new(
                "unnecessary-null-aware-assignments",
                Severity::Warning,
                "Unnecessary null-aware assignment to null.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}
