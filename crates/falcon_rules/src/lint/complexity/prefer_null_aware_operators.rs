//! Flags `x == null ? null : x.y` (and the inverted `x != null ? x.y : null`).
//!
//! Guarding a member access with a null-check ternary is what the `?.`
//! null-aware operator does: `x?.y`. It is shorter and evaluates the receiver
//! only once. The rule fires when one branch is the `null` literal and the
//! other is a field access, index, or method call whose receiver matches the
//! checked operand (whitespace-insensitive).

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferNullAwareOperators;

impl Rule for PreferNullAwareOperators {
    fn name(&self) -> &'static str {
        "prefer-null-aware-operators"
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

/// Return the non-null operand of a `E == null` / `E != null` comparison, along
/// with whether it was the equality (`==`) form.
fn null_check(cond: &Expr) -> Option<(&Expr, bool)> {
    let Expr::Binary {
        op, left, right, ..
    } = cond
    else {
        return None;
    };
    let is_eq = match op {
        BinaryOp::EqEq => true,
        BinaryOp::NotEq => false,
        _ => return None,
    };
    match (left.as_ref(), right.as_ref()) {
        (Expr::NullLit { .. }, other) | (other, Expr::NullLit { .. }) => Some((other, is_eq)),
        _ => None,
    }
}

/// The receiver of a direct member access, index, or method call — the part
/// that `?.` would guard.
fn access_receiver(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Field { object, .. } | Expr::Index { object, .. } => Some(object),
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field { object, .. } | Expr::Index { object, .. } => Some(object),
            _ => None,
        },
        _ => None,
    }
}

impl Collector<'_> {
    fn text(&self, e: &Expr) -> String {
        self.source[e.span().start..e.span().end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            span,
        } = node
            && let Some((operand, is_eq)) = null_check(condition)
        {
            // `x == null ? null : x.y` — null branch is `then`, access is `else`;
            // `x != null ? x.y : null` — access is `then`, null branch is `else`.
            let (null_branch, access_branch) = if is_eq {
                (then_expr, else_expr)
            } else {
                (else_expr, then_expr)
            };
            if matches!(null_branch.as_ref(), Expr::NullLit { .. })
                && let Some(receiver) = access_receiver(access_branch)
                && self.text(receiver) == self.text(operand)
            {
                self.diags.push(Diagnostic::new(
                    "prefer-null-aware-operators",
                    Severity::Warning,
                    "Prefer using the null-aware operator '?.'.",
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
