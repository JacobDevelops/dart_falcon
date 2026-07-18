//! Flags `return`/`break`/`continue` that escape a `finally` block, ported from
//! package:lints `control_flow_in_finally`. Control flow leaving a finally
//! silently discards any exception in flight. Breaks and continues that target
//! a loop or switch *inside* the finally are fine — only flow escaping the
//! finally is reported — and closures inside the finally are left alone.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct ControlFlowInFinally;

impl Rule for ControlFlowInFinally {
    fn name(&self) -> &'static str {
        "control-flow-in-finally"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for body in bodies(program) {
            find_finally(body, &mut diags, ctx);
        }
        diags
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

/// Depths of enclosing constructs *within the finally*. `breakable` counts
/// loops and switches (valid `break` targets); `loops` counts loops only
/// (valid `continue` targets).
#[derive(Clone, Copy)]
struct Depth {
    breakable: usize,
    loops: usize,
}

fn scan_finally(stmt: &Stmt, d: Depth, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Return(r) => flag(&r.span, diags, ctx),
        Stmt::Break(b) => {
            if b.label.is_none() && d.breakable == 0 {
                flag(&b.span, diags, ctx);
            }
        }
        Stmt::Continue(c) => {
            if c.label.is_none() && d.loops == 0 {
                flag(&c.span, diags, ctx);
            }
        }
        Stmt::Block(b) => {
            for s in &b.stmts {
                scan_finally(s, d, diags, ctx);
            }
        }
        Stmt::If(i) => {
            scan_finally(&i.then_branch, d, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_finally(eb, d, diags, ctx);
            }
        }
        Stmt::For(f) => scan_finally(
            &f.body,
            Depth {
                breakable: d.breakable + 1,
                loops: d.loops + 1,
            },
            diags,
            ctx,
        ),
        Stmt::While(w) => scan_finally(
            &w.body,
            Depth {
                breakable: d.breakable + 1,
                loops: d.loops + 1,
            },
            diags,
            ctx,
        ),
        Stmt::DoWhile(w) => scan_finally(
            &w.body,
            Depth {
                breakable: d.breakable + 1,
                loops: d.loops + 1,
            },
            diags,
            ctx,
        ),
        Stmt::Switch(sw) => {
            let inner = Depth {
                breakable: d.breakable + 1,
                loops: d.loops,
            };
            for case in &sw.cases {
                for s in &case.body {
                    scan_finally(s, inner, diags, ctx);
                }
            }
        }
        Stmt::TryCatch(tc) => {
            // The nested `finally` is scanned in its own right by `find_finally`.
            for s in &tc.body.stmts {
                scan_finally(s, d, diags, ctx);
            }
            for catch in &tc.catches {
                for s in &catch.body.stmts {
                    scan_finally(s, d, diags, ctx);
                }
            }
        }
        // Closures introduce their own control-flow scope — do not descend.
        _ => {}
    }
}

/// Walk the whole tree to locate every `finally` block, then scan each for
/// escaping control flow.
fn find_finally(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => find_in_stmts(&b.stmts, diags, ctx),
        FunctionBody::Arrow(e, _) => find_in_expr(e, diags, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn find_in_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        find_in_stmt(s, diags, ctx);
    }
}

fn find_in_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::TryCatch(tc) => {
            find_in_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                find_in_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                let d = Depth {
                    breakable: 0,
                    loops: 0,
                };
                for s in &fin.stmts {
                    scan_finally(s, d, diags, ctx);
                }
                // Recurse to catch nested finally blocks and closures within.
                find_in_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::If(i) => {
            find_in_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                find_in_stmt(eb, diags, ctx);
            }
        }
        Stmt::Block(b) => find_in_stmts(&b.stmts, diags, ctx),
        Stmt::While(w) => find_in_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => find_in_stmt(&d.body, diags, ctx),
        Stmt::For(f) => find_in_stmt(&f.body, diags, ctx),
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                find_in_stmts(&case.body, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => find_finally(&lf.body, diags, ctx),
        Stmt::Expr(e) => find_in_expr(&e.expr, diags, ctx),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                find_in_expr(v, diags, ctx);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    find_in_expr(init, diags, ctx);
                }
            }
        }
        _ => {}
    }
}

fn find_in_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::FuncExpr { body, .. } => find_finally(body, diags, ctx),
        Expr::Call { callee, args, .. } => {
            find_in_expr(callee, diags, ctx);
            for a in &args.positional {
                find_in_expr(a, diags, ctx);
            }
            for n in &args.named {
                find_in_expr(&n.value, diags, ctx);
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
