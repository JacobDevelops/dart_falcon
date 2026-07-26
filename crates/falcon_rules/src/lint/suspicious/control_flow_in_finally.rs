//! Flags `return`, `break`, or `continue` that escapes a `finally` block.
//!
//! Control flow leaving a finally overrides whatever the try or catch was doing,
//! including silently discarding an exception that was still propagating — the
//! error simply vanishes — and masking any value the try was about to return.
//! Keep finally blocks limited to cleanup and let exceptions and returns flow
//! through. A `break` or `continue` targeting a loop or switch declared *inside*
//! the finally is fine and left alone, as are closures defined within it; only
//! flow that escapes the finally itself is reported.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

pub struct ControlFlowInFinally;

impl Rule for ControlFlowInFinally {
    fn name(&self) -> &'static str {
        "control-flow-in-finally"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut finder = Finder {
            diags: Vec::new(),
            ctx,
        };
        finder.visit_program(program);
        finder
            .diags
            .sort_unstable_by_key(|diagnostic| diagnostic.span.start);
        finder
            .diags
            .dedup_by_key(|diagnostic| diagnostic.span.start);
        finder.diags
    }
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "control-flow-in-finally",
        Severity::Warning,
        "Avoid control flow (return/break/continue) that escapes a finally block",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

struct Finder<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Finder<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(try_catch) = node
            && let Some(finally) = &try_catch.finally
        {
            let mut scanner = FinallyScanner {
                diags: &mut self.diags,
                ctx: self.ctx,
                breakable: 0,
                loops: 0,
                break_labels: HashSet::new(),
                loop_labels: HashSet::new(),
            };
            for stmt in &finally.stmts {
                scanner.visit_stmt(stmt);
            }
        }
        walk_stmt(self, node);
    }
}

struct FinallyScanner<'a, 'ctx, 'diags> {
    diags: &'diags mut Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    breakable: usize,
    loops: usize,
    break_labels: HashSet<String>,
    loop_labels: HashSet<String>,
}

impl FinallyScanner<'_, '_, '_> {
    fn with_depth(&mut self, breakable: usize, loops: usize, walk: impl FnOnce(&mut Self)) {
        self.breakable += breakable;
        self.loops += loops;
        walk(self);
        self.breakable -= breakable;
        self.loops -= loops;
    }

    fn with_label(&mut self, labeled: &LabeledStmt, walk: impl FnOnce(&mut Self)) {
        let name = labeled.label.name.clone();
        self.break_labels.insert(name.clone());
        let mut target = labeled.stmt.as_ref();
        while let Stmt::Labeled(inner) = target {
            target = inner.stmt.as_ref();
        }
        let is_loop = matches!(target, Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_));
        if is_loop {
            self.loop_labels.insert(name.clone());
        }
        walk(self);
        self.break_labels.remove(&name);
        self.loop_labels.remove(&name);
    }

    fn visit_switch(&mut self, switch: &SwitchStmt) {
        self.visit_expr(&switch.subject);
        self.with_depth(1, 0, |this| {
            for label in switch.cases.iter().flat_map(|case| &case.labels) {
                this.loop_labels.insert(label.name.clone());
            }
            for case in &switch.cases {
                for kind in &case.cases {
                    if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                        this.visit_pattern(pattern);
                        if let Some(guard) = &**guard {
                            this.visit_expr(guard);
                        }
                    }
                }
                for stmt in &case.body {
                    this.visit_stmt(stmt);
                }
            }
            for label in switch.cases.iter().flat_map(|case| &case.labels) {
                this.loop_labels.remove(&label.name);
            }
        });
    }
}

impl Visitor for FinallyScanner<'_, '_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Return(ret) => flag(&ret.span, self.diags, self.ctx),
            Stmt::Break(brk) => {
                let stays_inside = brk.label.as_ref().map_or(self.breakable > 0, |label| {
                    self.break_labels.contains(&label.name)
                });
                if !stays_inside {
                    flag(&brk.span, self.diags, self.ctx);
                }
            }
            Stmt::Continue(cont) => {
                let stays_inside = cont.label.as_ref().map_or(self.loops > 0, |label| {
                    self.loop_labels.contains(&label.name)
                });
                if !stays_inside {
                    flag(&cont.span, self.diags, self.ctx);
                }
            }
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => {
                self.with_depth(1, 1, |this| walk_stmt(this, node));
            }
            Stmt::Switch(switch) => self.visit_switch(switch),
            Stmt::Labeled(labeled) => {
                self.with_label(labeled, |this| walk_stmt(this, node));
            }
            Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if !matches!(node, Expr::FuncExpr { .. }) {
            walk_expr(self, node);
        }
    }
}
