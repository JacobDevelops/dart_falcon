//! Flags `catch (e) { ... throw e; }` where the thrown value is exactly the caught
//! exception. Ported from package:lints `use_rethrow_when_possible`: such a throw should
//! be `rethrow`, which preserves the original stack trace.
//!
//! Conservative: a throw is only reported when `rethrow` would be syntactically valid and
//! refer to the same exception — i.e. it sits in the catch body's own control flow. Throws
//! inside a nested closure or a nested `try`/`catch` are skipped (there the exception is a
//! different binding, or `rethrow` is not allowed).

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program, walk_stmt};

pub struct UseRethrowWhenPossible;

impl Rule for UseRethrowWhenPossible {
    fn name(&self) -> &'static str {
        "use-rethrow-when-possible"
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
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(tc) = node {
            for catch in &tc.catches {
                if let Some(var) = &catch.exception_var {
                    let mut scan = CatchScan {
                        target: &var.name,
                        file: &self.file,
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
    matches!(expr, Expr::Ident(id) if id.name == target)
}

/// Walks a single catch body looking for `throw <exception>`. Stops at closures and
/// nested try/catch statements, where `rethrow` would not apply to `target`.
struct CatchScan<'a> {
    target: &'a str,
    file: &'a str,
    diags: &'a mut Vec<Diagnostic>,
}

impl CatchScan<'_> {
    fn flag(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "use-rethrow-when-possible",
            Severity::Warning,
            "Use `rethrow` to re-throw the caught exception and preserve its stack trace.",
            self.file.to_string(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for CatchScan<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Throw(t) if !t.is_rethrow && is_target(&t.value, self.target) => {
                self.flag(&t.span);
            }
            // Do not descend: a different exception binding / rethrow not valid here.
            Stmt::TryCatch(_) | Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Throw { expr, span } if is_target(expr, self.target) => {
                self.flag(span);
            }
            // Closures capture the exception but `rethrow` is illegal inside them.
            Expr::FuncExpr { .. } => {}
            _ => walk_expr(self, node),
        }
    }
}
