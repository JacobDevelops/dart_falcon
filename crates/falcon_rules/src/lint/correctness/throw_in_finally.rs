//! Flags `throw` inside a `finally` block.
//!
//! Throwing from a finally replaces whatever the try or catch was doing: an
//! exception that was still propagating is silently discarded and a value the
//! try was about to return is lost. Keep finally blocks limited to cleanup. A
//! throw inside a closure or local function defined in the finally is left
//! alone, as is a throw whose innermost enclosing `try` is nested inside the
//! finally (in its body or catch clauses).
//!
//! ```dart
//! try {
//!   work();
//! } finally {
//!   throw StateError('cleanup failed'); // flagged
//! }
//! ```
//!
//! Adopted from the official `throw_in_finally` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct ThrowInFinally;

impl Rule for ThrowInFinally {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut finder = Finder {
            ctx,
            diagnostics: Vec::new(),
            in_finally: false,
        };
        finder.visit_program(program);
        finder.diagnostics
    }
}

/// Tracks whether the innermost enclosing `try` statement (within the current
/// function body) has the visited node inside its `finally` block.
struct Finder<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diagnostics: Vec<Diagnostic>,
    in_finally: bool,
}

impl Finder<'_, '_> {
    fn with_finally(&mut self, in_finally: bool, walk: impl FnOnce(&mut Self)) {
        let saved = std::mem::replace(&mut self.in_finally, in_finally);
        walk(self);
        self.in_finally = saved;
    }

    fn report(&mut self, span: Span) {
        if self.in_finally {
            self.diagnostics.push(self.ctx.diagnostic(
                "throw-in-finally",
                "Use of 'throw' in a 'finally' block.",
                span,
            ));
        }
    }

    /// Visits a `try` statement: its body and catch clauses are outside any
    /// finally of this (now innermost) `try`; its finally block is inside one.
    fn visit_try(&mut self, try_catch: &TryCatchStmt) {
        self.with_finally(false, |this| {
            for stmt in &try_catch.body.stmts {
                this.visit_stmt(stmt);
            }
            for catch in &try_catch.catches {
                for stmt in &catch.body.stmts {
                    this.visit_stmt(stmt);
                }
            }
        });
        if let Some(finally) = &try_catch.finally {
            self.with_finally(true, |this| {
                for stmt in &finally.stmts {
                    this.visit_stmt(stmt);
                }
            });
        }
    }
}

impl Visitor for Finder<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::TryCatch(try_catch) => self.visit_try(try_catch),
            Stmt::Throw(throw) => {
                self.report(Span {
                    start: throw.span.start,
                    end: throw.value.span().end,
                });
                walk_stmt(self, node);
            }
            Stmt::LocalFunc(_) => self.with_finally(false, |this| walk_stmt(this, node)),
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Throw { span, .. } => {
                self.report(*span);
                walk_expr(self, node);
            }
            Expr::FuncExpr { .. } => self.with_finally(false, |this| walk_expr(this, node)),
            _ => walk_expr(self, node),
        }
    }
}
