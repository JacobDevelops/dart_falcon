//! Flags an expression statement that is a cascade with a single section.
//!
//! A cascade (`..`) exists to chain several operations on one receiver; with a
//! single section it buys nothing over an ordinary member access and only
//! obscures intent. Rewrite `foo..bar()` as `foo.bar()`. Cascades with two or
//! more sections are the idiomatic form and are not flagged.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidSingleCascadeInExpressionStatements;

impl Rule for AvoidSingleCascadeInExpressionStatements {
    fn name(&self) -> &'static str {
        "avoid-single-cascade-in-expression-statements"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for body in bodies(program) {
            walk_body(body, &mut diags, ctx);
        }
        diags
    }
}

fn check_expr_stmt(stmt: &ExprStmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Expr::Cascade { sections, span, .. } = &stmt.expr
        && sections.len() == 1
    {
        diags.push(Diagnostic::new(
            "avoid-single-cascade-in-expression-statements",
            Severity::Warning,
            "Single cascade in an expression statement — use `.` instead of `..`",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn walk_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => walk_stmts(&b.stmts, diags, ctx),
        FunctionBody::Arrow(_, _) => {}
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
        Stmt::Expr(e) => {
            check_expr_stmt(e, diags, ctx);
            walk_expr(&e.expr, diags, ctx);
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
        Stmt::TryCatch(tc) => {
            walk_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                walk_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                walk_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => walk_body(&lf.body, diags, ctx),
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
