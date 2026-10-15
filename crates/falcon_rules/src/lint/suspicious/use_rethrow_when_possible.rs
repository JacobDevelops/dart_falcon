//! Flags `throw e` inside a `catch (e)` by matching the exception parameter's name.
//!
//! Re-throwing the caught object with `throw e` starts a fresh stack trace from
//! the catch site, discarding the original point of failure and making the error
//! far harder to trace. Use `rethrow`, which propagates the same exception with
//! its trace intact. Matching is syntactic: the thrown identifier must have the
//! exception parameter's spelling; binding identity is not resolved. Throws
//! inside a nested closure or a nested `try`/`catch` are skipped.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct UseRethrowWhenPossible;

impl Rule for UseRethrowWhenPossible {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(tc) = node {
            for catch in &tc.catches {
                if let Some(var) = &catch.exception_var {
                    let mut scan = CatchScan {
                        target: &var.name,
                        ctx: self.ctx,
                        diags: &mut self.diags,
                    };
                    for stmt in &catch.body.stmts {
                        scan.visit_stmt(stmt);
                    }
                }
            }
        }
        // Continue walking so nested try/catch statements (including those inside a
        // catch body, which `CatchScan` deliberately skips) get their own analysis.
        walk_stmt(self, node);
    }
}

fn is_target(expr: &Expr, target: &str) -> bool {
    matches!(expr.unparenthesized(), Expr::Ident(id) if id.name == target)
}

/// Walks a single catch body looking for `throw <exception>`. Stops at closures and
/// nested try/catch statements, where `rethrow` would not apply to `target`.
struct CatchScan<'a> {
    target: &'a str,
    ctx: &'a AnalyzeContext<'a>,
    diags: &'a mut Vec<Diagnostic>,
}

impl Visitor for CatchScan<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Throw(t) if is_target(&t.value, self.target) => {
                self.diags.push(self.ctx.diagnostic(
                    "use-rethrow-when-possible",
                    "Use `rethrow` to re-throw the caught exception and preserve its stack trace.",
                    t.span,
                ));
            }
            // Do not descend: a different exception binding / rethrow not valid here.
            Stmt::TryCatch(_) | Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Throw { expr, span } if is_target(expr, self.target) => {
                self.diags.push(self.ctx.diagnostic(
                    "use-rethrow-when-possible",
                    "Use `rethrow` to re-throw the caught exception and preserve its stack trace.",
                    *span,
                ));
            }
            // Closures capture the exception but `rethrow` is illegal inside them.
            Expr::FuncExpr { .. } => {}
            _ => walk_expr(self, node),
        }
    }
}
