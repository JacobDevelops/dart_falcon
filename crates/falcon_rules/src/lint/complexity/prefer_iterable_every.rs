//! Flags `!iterable.where(...).isEmpty` and `iterable.where(...).length == other.length`.
//!
//! Both patterns ask whether every element satisfies a predicate;
//! `iterable.every(...)` says so directly and short-circuits on the first
//! failure. The rule matches a negated `.where(...).isEmpty` and a
//! `.where(...).length` compared for equality against a `.length`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferIterableEvery;

impl Rule for PreferIterableEvery {
    fn name(&self) -> &'static str {
        "prefer-iterable-every"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

fn is_negated_where_is_empty(expr: &Expr) -> Option<Span> {
    if let Expr::Unary {
        op: UnaryOp::Bang,
        operand,
        span,
    } = expr
        && let Expr::Field { object, field, .. } = &**operand
        && field.name == "isEmpty"
        && let Expr::Call { callee, .. } = &**object
        && let Expr::Field {
            field: where_field, ..
        } = &**callee
        && where_field.name == "where"
    {
        return Some(Span {
            start: span.start,
            end: span.end,
        });
    }
    None
}

fn is_where_length_eq_length(expr: &Expr) -> Option<Span> {
    if let Expr::Binary {
        op: BinaryOp::EqEq,
        left,
        right,
        span,
    } = expr
        && let Expr::Field {
            object: left_obj,
            field: left_field,
            ..
        } = &**left
        && left_field.name == "length"
        && let Expr::Call {
            callee: left_callee,
            ..
        } = &**left_obj
        && let Expr::Field {
            field: left_where_field,
            ..
        } = &**left_callee
        && left_where_field.name == "where"
        && let Expr::Field {
            field: right_field, ..
        } = &**right
        && right_field.name == "length"
    {
        return Some(Span {
            start: span.start,
            end: span.end,
        });
    }
    None
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "prefer-iterable-every",
        Severity::Warning,
        "Use .every() instead of .where().isEmpty or .where().length comparison.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Some(span) =
            is_negated_where_is_empty(node).or_else(|| is_where_length_eq_length(node))
        {
            flag(&span, &mut self.diags, self.ctx);
        }
        walk_expr(self, node);
    }
}
