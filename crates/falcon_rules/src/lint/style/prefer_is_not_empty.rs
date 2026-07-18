//! Flags `.length` comparisons equivalent to `.isNotEmpty` (`length != 0`,
//! `length > 0`, `length >= 1`, and their mirrors). Adopted from package:lints
//! `prefer_is_not_empty`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferIsNotEmpty;

impl Rule for PreferIsNotEmpty {
    fn name(&self) -> &'static str {
        "prefer-is-not-empty"
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
            && is_not_empty_comparison(op, left, right)
        {
            self.diags.push(Diagnostic::new(
                "prefer-is-not-empty",
                Severity::Warning,
                "Use 'isNotEmpty' instead of comparing 'length' to 0.",
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

/// True for `.length` comparisons equivalent to `isNotEmpty`.
fn is_not_empty_comparison(op: &BinaryOp, left: &Expr, right: &Expr) -> bool {
    match op {
        // length != 0 / 0 != length
        BinaryOp::NotEq => {
            (is_length(left) && is_int(right, 0)) || (is_int(left, 0) && is_length(right))
        }
        // length > 0
        BinaryOp::Gt => is_length(left) && is_int(right, 0),
        // 0 < length
        BinaryOp::Lt => is_int(left, 0) && is_length(right),
        // length >= 1
        BinaryOp::GtEq => is_length(left) && is_int(right, 1),
        // 1 <= length
        BinaryOp::LtEq => is_int(left, 1) && is_length(right),
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
