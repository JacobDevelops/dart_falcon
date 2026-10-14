//! Flags `iterable.where(...).isNotEmpty`.
//!
//! Testing whether a filtered iterable is non-empty walks the filter just to
//! check for any match; `iterable.any(...)` short-circuits on the first match
//! and reads better. The rule matches an `.isNotEmpty` access on the result of a
//! `.where(...)` call.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct PreferIterableAny;

impl Rule for PreferIterableAny {
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
        return Some(*span);
    }
    None
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the AST, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some(span) = is_where_is_not_empty(node) {
            self.diags.push(self.ctx.diagnostic(
                "prefer-iterable-any",
                "Use .any() instead of .where().isNotEmpty.",
                span,
            ));
        }
        walk_expr(self, node);
    }
}
