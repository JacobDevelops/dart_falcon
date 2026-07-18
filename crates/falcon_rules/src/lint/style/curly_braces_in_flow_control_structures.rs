//! Requires curly braces around the bodies of flow-control statements.
//!
//! A brace-less body invites the "goto fail" class of bug, where a later edit
//! adds a second statement that silently falls outside the branch. Requiring
//! blocks around `for`, `while`, `do`, and `if`/`else` bodies keeps the scope
//! explicit and edits safe. Two carve-outs match the official lint: an `if`
//! with no `else` whose body sits on the same line as its condition may omit
//! braces, and an `else if` chain need not wrap the intermediate `if` in a
//! block.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct CurlyBracesInFlowControlStructures;

impl Rule for CurlyBracesInFlowControlStructures {
    fn name(&self) -> &'static str {
        "curly-braces-in-flow-control-structures"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for body in bodies(program) {
            walk_body(body, &mut diags, ctx);
        }
        diags
    }
}

fn report(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let span = stmt.span();
    diags.push(Diagnostic::new(
        "curly-braces-in-flow-control-structures",
        Severity::Warning,
        "Use curly braces around the body of this flow-control statement",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// True when the body begins on the same source line as the condition's
/// closing `)`, mirroring the official single-line `if` exemption.
fn on_condition_line(body: &Stmt, ctx: &AnalyzeContext) -> bool {
    let start = body.span().start;
    match ctx.source[..start].rfind(')') {
        Some(p) => !ctx.source[p..start].contains('\n'),
        None => true,
    }
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
        Stmt::If(i) => {
            match &i.else_branch {
                None => {
                    if !matches!(&*i.then_branch, Stmt::Block(_))
                        && !on_condition_line(&i.then_branch, ctx)
                    {
                        report(&i.then_branch, diags, ctx);
                    }
                }
                Some(eb) => {
                    if !matches!(&*i.then_branch, Stmt::Block(_)) {
                        report(&i.then_branch, diags, ctx);
                    }
                    // An `else if` chain is fine; any other non-block else is not.
                    if !matches!(&**eb, Stmt::Block(_) | Stmt::If(_)) {
                        report(eb, diags, ctx);
                    }
                }
            }
            walk_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                walk_stmt(eb, diags, ctx);
            }
        }
        Stmt::For(f) => {
            if !matches!(&*f.body, Stmt::Block(_)) {
                report(&f.body, diags, ctx);
            }
            walk_stmt(&f.body, diags, ctx);
        }
        Stmt::While(w) => {
            if !matches!(&*w.body, Stmt::Block(_)) {
                report(&w.body, diags, ctx);
            }
            walk_stmt(&w.body, diags, ctx);
        }
        Stmt::DoWhile(d) => {
            if !matches!(&*d.body, Stmt::Block(_)) {
                report(&d.body, diags, ctx);
            }
            walk_stmt(&d.body, diags, ctx);
        }
        Stmt::Block(b) => walk_stmts(&b.stmts, diags, ctx),
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
