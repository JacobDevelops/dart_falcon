//! Flags `.length` comparisons equivalent to `.isEmpty` (`length == 0`,
//! `length < 1`, `length <= 0`, and their mirrors). Adopted from package:lints
//! `prefer_is_empty`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferIsEmpty;

impl Rule for PreferIsEmpty {
    fn name(&self) -> &'static str {
        "prefer-is-empty"
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
            op,
            left,
            right,
            span,
        } = node
            && is_empty_comparison(op, left, right)
        {
            self.diags.push(Diagnostic::new(
                "prefer-is-empty",
                Severity::Warning,
                "Use 'isEmpty' instead of comparing 'length' to 0.",
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

/// True for `.length` comparisons equivalent to `isEmpty`.
fn is_empty_comparison(op: &BinaryOp, left: &Expr, right: &Expr) -> bool {
    match op {
        // length == 0 / 0 == length
        BinaryOp::EqEq => {
            (is_length(left) && is_int(right, 0)) || (is_int(left, 0) && is_length(right))
        }
        // length < 1
        BinaryOp::Lt => is_length(left) && is_int(right, 1),
        // 1 > length
        BinaryOp::Gt => is_int(left, 1) && is_length(right),
        // length <= 0
        BinaryOp::LtEq => is_length(left) && is_int(right, 0),
        // 0 >= length
        BinaryOp::GtEq => is_int(left, 0) && is_length(right),
        _ => false,
    }
}

/// True for a `.length` property access.
fn is_length(expr: &Expr) -> bool {
    matches!(expr, Expr::Field { field, .. } if field.name == "length")
}

/// True for an integer literal equal to `n`.
fn is_int(expr: &Expr, n: i64) -> bool {
    if let Expr::IntLit { value, .. } = expr {
        value.replace('_', "").parse::<i64>().ok() == Some(n)
    } else {
        false
    }
}
