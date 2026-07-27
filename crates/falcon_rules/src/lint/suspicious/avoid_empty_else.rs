//! Flags an `else` clause whose body is an empty statement (`else ;`).
//!
//! An `else` immediately followed by a semicolon binds that empty statement as
//! the whole else branch, so the block meant to run instead executes
//! unconditionally right after the `if`. This is nearly always a stray semicolon
//! typed between `else` and the intended `{ ... }`. Remove the semicolon so the
//! following block becomes the else body, or delete the `else` entirely if no
//! alternative branch is needed.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_stmt};

pub struct AvoidEmptyElse;

impl Rule for AvoidEmptyElse {
    fn name(&self) -> &'static str {
        "avoid-empty-else"
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

/// An empty statement is parsed as an `Expr` statement wrapping a `NullLit`
/// whose span begins at the `;` token; a real `null;` starts at `null`.
fn is_empty_semicolon(stmt: &Stmt, ctx: &AnalyzeContext) -> bool {
    if let Stmt::Expr(e) = stmt
        && let Expr::NullLit { span } = &e.expr
    {
        return ctx.source.as_bytes().get(span.start) == Some(&b';');
    }
    false
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::If(if_stmt) = node
            && let Some(else_branch) = &if_stmt.else_branch
            && is_empty_semicolon(else_branch, self.ctx)
        {
            let span = else_branch.span();
            self.diags.push(Diagnostic::new(
                "avoid-empty-else",
                Severity::Warning,
                "Empty `else` clause — remove the `else` or give it a body",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_stmt(self, node);
    }
}
