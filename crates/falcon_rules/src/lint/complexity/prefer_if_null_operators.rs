//! Flags `x == null ? y : x` (and the inverted `x != null ? x : y`) conditional
//! expressions that are better written with the `??` operator. Ported from
//! dart_code_linter's `prefer-if-null-operators`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferIfNullOperators;

impl Rule for PreferIfNullOperators {
    fn name(&self) -> &'static str {
        "prefer-if-null-operators"
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
            // `x == null ? y : x` keeps `x` in the else branch;
            // `x != null ? x : y` keeps `x` in the then branch.
            let fallthrough = if is_eq { else_expr } else { then_expr };
            if self.text(operand) == self.text(fallthrough) {
                self.diags.push(Diagnostic::new(
                    "prefer-if-null-operators",
                    Severity::Warning,
                    "Prefer using the if-null operator '??'.",
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
