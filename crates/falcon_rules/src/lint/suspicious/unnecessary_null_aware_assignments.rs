//! Flags `x ??= null;`, which has no effect. Ported from package:lints'
//! `unnecessary_null_aware_assignments`.

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
