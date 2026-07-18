//! Flags empty `catch` blocks.
//!
//! A catch clause with no body silently swallows the exception, hiding failures
//! that should be handled, logged, or rethrown and making bugs far harder to
//! diagnose. Handle the error, or at minimum log it, rather than discarding it.
//! Two escape hatches match the official lint: a catch whose body contains a
//! comment (the emptiness is deliberate and documented), and a catch that binds
//! its exception to `_`, an explicit "I am ignoring this" marker.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct EmptyCatches;

impl Rule for EmptyCatches {
    fn name(&self) -> &'static str {
        "empty-catches"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for body in bodies(program) {
            walk_body(body, &mut diags, ctx);
        }
        diags
    }
}

fn check_catch(catch: &CatchClause, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !catch.body.stmts.is_empty() {
        return;
    }
    // Exempt `catch (_) {}` / `on E catch (_) {}`.
    if catch.exception_var.as_ref().is_some_and(|v| v.name == "_") {
        return;
    }
    // Exempt an intentionally documented empty body.
    let end = catch.body.span.end.min(ctx.source.len());
    let inner = &ctx.source[catch.body.span.start..end];
    let close = match inner.rfind('}') {
        Some(p) => p,
        None => return,
    };
    let braces = &inner[..=close];
    if braces.contains("//") || braces.contains("/*") {
        return;
    }
    let close_byte = catch.body.span.start + close;
    diags.push(Diagnostic::new(
        "empty-catches",
        Severity::Warning,
        "Empty catch block — handle the exception, add a comment, or name it `_`",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: close_byte,
            end: close_byte + 1,
        },
    ));
}

fn walk_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => walk_stmts(&b.stmts, diags, ctx),
        FunctionBody::Arrow(e, _) => walk_expr(e, diags, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn walk_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        walk_stmt(s, diags, ctx);
    }
}

fn walk_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::TryCatch(tc) => {
            walk_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                check_catch(catch, diags, ctx);
                walk_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                walk_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::If(i) => {
            walk_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                walk_stmt(eb, diags, ctx);
            }
        }
        Stmt::Block(b) => walk_stmts(&b.stmts, diags, ctx),
        Stmt::While(w) => walk_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => walk_stmt(&d.body, diags, ctx),
        Stmt::For(f) => walk_stmt(&f.body, diags, ctx),
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                walk_stmts(&case.body, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => walk_body(&lf.body, diags, ctx),
        Stmt::Expr(e) => walk_expr(&e.expr, diags, ctx),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                walk_expr(v, diags, ctx);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    walk_expr(init, diags, ctx);
                }
            }
        }
        _ => {}
    }
}

fn walk_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::FuncExpr { body, .. } => walk_body(body, diags, ctx),
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, diags, ctx);
            for a in &args.positional {
                walk_expr(a, diags, ctx);
            }
            for n in &args.named {
                walk_expr(&n.value, diags, ctx);
            }
        }
        _ => {}
    }
}

/// Every function body reachable from top-level declarations and their members.
fn bodies(program: &Program) -> Vec<&FunctionBody> {
    let mut out = Vec::new();
    for decl in &program.declarations {
        match decl {
            TopLevelDecl::Function(f) => out.extend(f.body.as_ref()),
            TopLevelDecl::Class(c) => member_bodies(&c.members, &mut out),
            TopLevelDecl::Mixin(m) => member_bodies(&m.members, &mut out),
            TopLevelDecl::MixinClass(mc) => member_bodies(&mc.members, &mut out),
            TopLevelDecl::Enum(e) => member_bodies(&e.members, &mut out),
            TopLevelDecl::Extension(e) => member_bodies(&e.members, &mut out),
            TopLevelDecl::ExtensionType(e) => member_bodies(&e.members, &mut out),
            _ => {}
        }
    }
    out
}

fn member_bodies<'a>(members: &'a [ClassMember], out: &mut Vec<&'a FunctionBody>) {
    for m in members {
        let body = match m {
            ClassMember::Method(x) => x.body.as_ref(),
            ClassMember::Constructor(x) => x.body.as_ref(),
            ClassMember::Getter(x) => x.body.as_ref(),
            ClassMember::Setter(x) => x.body.as_ref(),
            ClassMember::Operator(x) => x.body.as_ref(),
            _ => None,
        };
        out.extend(body);
    }
}
