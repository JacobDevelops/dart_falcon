//! Flags a `throw` of a new object inside a `catch` block.
//!
//! Throwing a fresh exception from a catch replaces the error in flight along
//! with its original stack trace, so the root cause is lost and the failure
//! becomes much harder to diagnose. Prefer `rethrow` to propagate the caught
//! exception unchanged, or wrap it in an error that keeps the original as a
//! cause. Only a `throw` statement raising a new value is reported; a `rethrow`
//! is always exempt. Delayed local functions and closures are separate execution
//! scopes and are not treated as part of the surrounding catch.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

pub struct AvoidThrowInCatchBlock;

impl Rule for AvoidThrowInCatchBlock {
    fn name(&self) -> &'static str {
        "avoid-throw-in-catch-block"
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

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(try_catch) = node {
            for catch in &try_catch.catches {
                let mut catch_scan = CatchScan {
                    diags: &mut self.diags,
                    ctx: self.ctx,
                };
                for stmt in &catch.body.stmts {
                    catch_scan.visit_stmt(stmt);
                }
            }
        }
        walk_stmt(self, node);
    }
}

struct CatchScan<'a, 'ctx> {
    diags: &'a mut Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for CatchScan<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if matches!(node, Stmt::LocalFunc(_)) {
            return;
        }
        if let Stmt::TryCatch(try_catch) = node {
            for stmt in &try_catch.body.stmts {
                self.visit_stmt(stmt);
            }
            if let Some(finally) = &try_catch.finally {
                for stmt in &finally.stmts {
                    self.visit_stmt(stmt);
                }
            }
            return;
        }
        if let Stmt::Throw(throw_stmt) = node
            && !throw_stmt.is_rethrow
        {
            self.diags.push(Diagnostic::new(
                "avoid-throw-in-catch-block",
                Severity::Warning,
                "Avoid throwing exceptions within catch blocks",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: throw_stmt.span.start,
                    end: throw_stmt.span.end,
                },
            ));
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if !matches!(node, Expr::FuncExpr { .. }) {
            walk_expr(self, node);
        }
    }
}
