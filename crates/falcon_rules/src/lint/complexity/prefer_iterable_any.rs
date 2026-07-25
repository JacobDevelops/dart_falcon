//! Flags `iterable.where(...).isNotEmpty`.
//!
//! Testing whether a filtered iterable is non-empty walks the filter just to
//! check for any match; `iterable.any(...)` short-circuits on the first match
//! and reads better. The rule matches an `.isNotEmpty` access on the result of a
//! `.where(...)` call.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferIterableAny;

impl Rule for PreferIterableAny {
    fn name(&self) -> &'static str {
        "prefer-iterable-any"
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

fn is_where_is_not_empty(expr: &Expr) -> Option<Span> {
    // Match: something.where(...).isNotEmpty
    if let Expr::Field {
        object,
        field,
        span,
        ..
    } = expr
        && field.name == "isNotEmpty"
        && let Expr::Call { callee, .. } = &**object
        && let Expr::Field {
            object: _where_object,
            field: where_field,
            ..
        } = &**callee
        && where_field.name == "where"
    {
        return Some(span.clone());
    }
    None
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "prefer-iterable-any",
        Severity::Warning,
        "Use .any() instead of .where().isNotEmpty.",
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
        if let Some(span) = is_where_is_not_empty(node) {
            flag(&span, &mut self.diags, self.ctx);
        }
        walk_expr(self, node);
    }
}
