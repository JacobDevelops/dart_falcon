//! Flags a function whose control-flow nesting runs deeper than the configured level.
//!
//! Deeply nested blocks push logic to the right and make control flow hard to
//! follow; early returns, guard clauses, and extracted helpers flatten it. The
//! rule measures the deepest nesting of control structures — `if`, `for`,
//! `while`, `do`, `switch`, and `try` — within a function body. A plain block
//! does not add a level, `else if` chains stay at the same level, and a nested
//! local function restarts the count. It reports at the function name when the
//! depth exceeds the threshold.
//!
//! ## Options
//!
//! `max_nesting` (integer, default: 5) — flag when the nesting depth exceeds
//! this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct MaximumNestingLevel;

impl Rule for MaximumNestingLevel {
    fn name(&self) -> &'static str {
        "maximum-nesting-level"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// Read the `max_nesting` option (default 5). Malformed/missing → default.
fn max_nesting_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("maximum-nesting-level")
        .and_then(|m| ctx.rule_options(m.group, "maximum-nesting-level"))
        .and_then(|o| o.get("max_nesting"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(5)
}

/// Compute the deepest control-flow nesting level within one function body and
/// emit a diagnostic at `name_span` when it exceeds the configured threshold.
///
/// Only control structures (if/for/while/do-while/switch/try) increase depth;
/// a plain block does not. Nested local functions start a fresh depth of 0.
fn check_function(
    body: &FunctionBody,
    name_span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let threshold = max_nesting_option(ctx);
    let mut max = 0;
    walk_body(body, 0, &mut max);
    let mut closures = ClosureNesting { max };
    match body {
        FunctionBody::Block(b) => {
            for s in &b.stmts {
                closures.visit_stmt(s);
            }
        }
        FunctionBody::Arrow(e, _) => closures.visit_expr(e),
        FunctionBody::Native(_, _) => {}
    }
    max = closures.max;
    if max > threshold {
        diags.push(Diagnostic::new(
            "maximum-nesting-level",
            Severity::Warning,
            format!("Function has a nesting level of {max} (max {threshold})."),
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: name_span.start,
                end: name_span.end,
            },
        ));
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                check_function(body, &f.name.span, diags, ctx);
            }
        }
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Extension(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::ExtensionType(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Enum(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Method(m) => {
            if let Some(b) = &m.body {
                check_function(b, &m.name.span, diags, ctx);
            }
        }
        ClassMember::Constructor(c) => {
            if let Some(b) = &c.body {
                check_function(b, &c.name.span, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            if let Some(b) = &g.body {
                check_function(b, &g.name.span, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            if let Some(b) = &s.body {
                check_function(b, &s.name.span, diags, ctx);
            }
        }
        _ => {}
    }
}

// ── Nesting-depth walk ────────────────────────────────────────────────────────

fn walk_body(body: &FunctionBody, depth: usize, max: &mut usize) {
    if let FunctionBody::Block(b) = body {
        for s in &b.stmts {
            walk_stmt(s, depth, max);
        }
    }
}

fn walk_stmt(stmt: &Stmt, depth: usize, max: &mut usize) {
    match stmt {
        Stmt::Block(b) => {
            // A bare block does not add a nesting level on its own.
            for s in &b.stmts {
                walk_stmt(s, depth, max);
            }
        }
        Stmt::If(i) => {
            let inner = depth + 1;
            *max = (*max).max(inner);
            walk_stmt(&i.then_branch, inner, max);
            if let Some(e) = &i.else_branch {
                match e.as_ref() {
                    // `else if` chains at the same level, not deeper.
                    Stmt::If(_) => walk_stmt(e, depth, max),
                    _ => walk_stmt(e, inner, max),
                }
            }
        }
        Stmt::For(f) => {
            let inner = depth + 1;
            *max = (*max).max(inner);
            walk_stmt(&f.body, inner, max);
        }
        Stmt::While(w) => {
            let inner = depth + 1;
            *max = (*max).max(inner);
            walk_stmt(&w.body, inner, max);
        }
        Stmt::DoWhile(d) => {
            let inner = depth + 1;
            *max = (*max).max(inner);
            walk_stmt(&d.body, inner, max);
        }
        Stmt::Switch(sw) => {
            let inner = depth + 1;
            *max = (*max).max(inner);
            for case in &sw.cases {
                for s in &case.body {
                    walk_stmt(s, inner, max);
                }
            }
        }
        Stmt::TryCatch(tc) => {
            let inner = depth + 1;
            *max = (*max).max(inner);
            for s in &tc.body.stmts {
                walk_stmt(s, inner, max);
            }
            for catch in &tc.catches {
                for s in &catch.body.stmts {
                    walk_stmt(s, inner, max);
                }
            }
            if let Some(fin) = &tc.finally {
                for s in &fin.stmts {
                    walk_stmt(s, inner, max);
                }
            }
        }
        // Nested local functions start their own fresh nesting context.
        Stmt::LocalFunc(lf) => walk_body(&lf.body, 0, max),
        // A label wraps a statement without adding depth; recurse into it.
        Stmt::Labeled(l) => walk_stmt(&l.stmt, depth, max),
        _ => {}
    }
}

/// Finds every function-expression (closure) anywhere in a body and measures
/// its nesting from a fresh depth of 0, mirroring how local functions restart
/// the count. Closures in argument position (`run(() { … })`) were otherwise
/// invisible to the statement walk.
struct ClosureNesting {
    max: usize,
}

impl Visitor for ClosureNesting {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { body, .. } = node {
            walk_body(body, 0, &mut self.max);
        }
        walk_expr(self, node);
    }
}
