//! Flags `'a' + 'b'` where both operands are string literals. Ported from package:lints
//! `prefer_adjacent_string_concatenation`. Two string literals should be written adjacent
//! (`'a' 'b'`) rather than joined with `+`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferAdjacentStringConcatenation;

impl Rule for PreferAdjacentStringConcatenation {
    fn name(&self) -> &'static str {
        "prefer-adjacent-string-concatenation"
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
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Binary {
            op: BinaryOp::Add,
            left,
            right,
            span,
        } = node
            && matches!(left.as_ref(), Expr::StringLit(_))
            && matches!(right.as_ref(), Expr::StringLit(_))
        {
            self.diags.push(Diagnostic::new(
                "prefer-adjacent-string-concatenation",
                Severity::Warning,
                "Use adjacent string literals instead of the `+` operator.",
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
