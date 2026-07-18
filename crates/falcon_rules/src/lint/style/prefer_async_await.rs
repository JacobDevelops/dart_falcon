//! Flags `.then(...)` future chains that should use `async`/`await`.
//!
//! Chaining callbacks with `Future.then` nests logic inside closures and scatters
//! error handling across `onError`/`catchError`, whereas `async`/`await` lets
//! asynchronous code read top-to-bottom with ordinary `try`/`catch`. The rule
//! walks statement and expression trees looking for any method call named `then`,
//! reporting each one; it treats function-literal arguments as opaque and does
//! not descend into lambda bodies. Matching is purely syntactic on the `then`
//! name, so it does not confirm the receiver is actually a `Future`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferAsyncAwait;

impl Rule for PreferAsyncAwait {
    fn name(&self) -> &'static str {
        "prefer-async-await"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    if let Some(FunctionBody::Block(block)) = &func.body {
                        scan_stmts(&block.stmts, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Class(class) => {
                    for member in &class.members {
                        scan_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Mixin(mixin) => {
                    for member in &mixin.members {
                        scan_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::MixinClass(mc) => {
                    for member in &mc.members {
                        scan_member(member, &mut diags, ctx);
                    }
                }
                _ => {}
            }
        }
        diags
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    };
    if let Some(FunctionBody::Block(block)) = body {
        scan_stmts(&block.stmts, diags, ctx);
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts {
        scan_stmt(stmt, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Expr(expr_stmt) => scan_expr(&expr_stmt.expr, diags, ctx),
        Stmt::Return(ret) => {
            if let Some(expr) = &ret.value {
                scan_expr(expr, diags, ctx);
            }
        }
        Stmt::LocalVar(local) => {
            for decl in &local.declarators {
                if let Some(init) = &decl.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        Stmt::Block(block) => scan_stmts(&block.stmts, diags, ctx),
        Stmt::If(if_stmt) => {
            scan_stmt(&if_stmt.then_branch, diags, ctx);
            if let Some(else_b) = &if_stmt.else_branch {
                scan_stmt(else_b, diags, ctx);
            }
        }
        Stmt::While(s) => scan_stmt(&s.body, diags, ctx),
        Stmt::DoWhile(s) => scan_stmt(&s.body, diags, ctx),
        Stmt::For(s) => scan_stmt(&s.body, diags, ctx),
        Stmt::TryCatch(s) => {
            scan_stmts(&s.body.stmts, diags, ctx);
            for catch in &s.catches {
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &s.finally {
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Call {
            callee, args, span, ..
        } => {
            if let Expr::Field { field, object, .. } = callee.as_ref() {
                if field.name == "then" {
                    diags.push(Diagnostic::new(
                        "prefer-async-await",
                        Severity::Warning,
                        "Prefer async/await over .then() chains",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
                scan_expr(object, diags, ctx);
            } else {
                scan_expr(callee, diags, ctx);
            }
            for arg in &args.positional {
                // Skip FuncExpr bodies — treat lambdas as opaque
                if !matches!(arg, Expr::FuncExpr { .. }) {
                    scan_expr(arg, diags, ctx);
                }
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
        Expr::Await { expr: inner, .. } => scan_expr(inner, diags, ctx),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            scan_expr(condition, diags, ctx);
            scan_expr(then_expr, diags, ctx);
            scan_expr(else_expr, diags, ctx);
        }
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
        }
        _ => {}
    }
}
