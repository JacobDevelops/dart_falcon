//! Flags comparisons of an expression with itself. Ported from pyramid_lint's `no_self_comparisons`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct NoSelfComparisons;

impl Rule for NoSelfComparisons {
    fn name(&self) -> &'static str {
        "no_self_comparisons"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    file: String,
    source: &'s str,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Binary {
            op,
            left,
            right,
            span,
        } = node
            && is_comparison(op)
        {
            let l = strip_ws(&self.source[left.span().start..left.span().end]);
            let r = strip_ws(&self.source[right.span().start..right.span().end]);
            if !l.is_empty() && l == r {
                self.diags.push(Diagnostic::new(
                    "no_self_comparisons",
                    Severity::Warning,
                    "Both operands of this comparison are identical.",
                    self.file.clone(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
        }
        walk_expr(self, node);
    }
}

fn is_comparison(op: &BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
    )
}

/// Collapse an operand's source text by removing all whitespace, so that
/// formatting differences do not hide a structurally identical comparison.
fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}
