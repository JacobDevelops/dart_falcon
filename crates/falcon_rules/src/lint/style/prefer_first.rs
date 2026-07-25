//! Flags `[0]` index access that should use the `.first` getter.
//!
//! Reading the leading element as `xs.first` states the intent directly and is
//! easier to read than the numeric `xs[0]`, which forces the reader to recognize
//! that zero means "first". The rule matches an index expression whose subscript
//! is the integer literal `0`; any other index, including a non-literal
//! expression that evaluates to zero, is left alone. Matching is syntactic on the
//! literal, so it does not confirm the receiver exposes a `first` getter.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferFirst;

impl Rule for PreferFirst {
    fn name(&self) -> &'static str {
        "prefer-first"
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
        if let Expr::Index { index, span, .. } = node
            && is_zero(index)
        {
            self.diags.push(Diagnostic::new(
                "prefer-first",
                Severity::Warning,
                "Prefer .first over [0] to access the first element",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}

/// True for the integer literal `0`.
fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::IntLit { value, .. } if value == "0")
}

