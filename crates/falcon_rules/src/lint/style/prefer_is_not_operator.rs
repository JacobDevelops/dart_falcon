//! Flags `!(x is T)`, which is better written as `x is! T`. Ported from
//! package:lints' `prefer_is_not_operator`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferIsNotOperator;

impl Rule for PreferIsNotOperator {
    fn name(&self) -> &'static str {
        "prefer-is-not-operator"
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
        if let Expr::Unary {
            op: UnaryOp::Bang,
            operand,
            span,
        } = node
            && matches!(operand.as_ref(), Expr::Is { negated: false, .. })
        {
            self.diags.push(Diagnostic::new(
                "prefer-is-not-operator",
                Severity::Warning,
                "Prefer using the 'is!' operator.",
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
