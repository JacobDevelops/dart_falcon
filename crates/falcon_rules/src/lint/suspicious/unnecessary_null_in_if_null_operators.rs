//! Flags `x ?? null` and `null ?? x`, where the `null` operand is redundant.
//!
//! The `??` operator falls back to its right operand only when the left is null,
//! so `x ?? null` always evaluates to `x` and `null ?? x` always evaluates to
//! `x`. The literal `null` contributes nothing and typically marks an unfinished
//! edit or a misunderstanding of the operator. Delete the `null` operand, and the
//! `??` along with it.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct UnnecessaryNullInIfNullOperators;

impl Rule for UnnecessaryNullInIfNullOperators {
    fn name(&self) -> &'static str {
        "unnecessary-null-in-if-null-operators"
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
        if let Expr::Binary {
            op: BinaryOp::NullCoalesce | BinaryOp::IfNull,
            left,
            right,
            span,
        } = node
            && (matches!(left.as_ref(), Expr::NullLit { .. })
                || matches!(right.as_ref(), Expr::NullLit { .. }))
        {
            self.diags.push(Diagnostic::new(
                "unnecessary-null-in-if-null-operators",
                Severity::Warning,
                "Unnecessary null in an if-null ('??') operator.",
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
