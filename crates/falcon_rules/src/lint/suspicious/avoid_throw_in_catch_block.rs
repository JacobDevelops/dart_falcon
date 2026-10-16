//! Flags `throw` statements inside a `catch` block.
//!
//! Throwing a fresh exception from a catch replaces the error in flight along
//! with its original stack trace, so the root cause is lost and the failure
//! becomes much harder to diagnose. Prefer `rethrow` to propagate the caught
//! exception unchanged, or wrap it in an error that keeps the original as a
//! cause. Every `throw` statement is reported, including throws of existing
//! values; `rethrow` is always exempt. Delayed local functions and closures are
//! separate execution scopes and are not part of the surrounding catch.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct AvoidThrowInCatchBlock;

impl Rule for AvoidThrowInCatchBlock {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
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
        if let Stmt::Throw(throw_stmt) = node {
            self.diags.push(self.ctx.diagnostic(
                "avoid-throw-in-catch-block",
                "Avoid throwing exceptions within catch blocks",
                throw_stmt.span,
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
