//! Flags `[0]` index access that should use the `.first` getter.
//!
//! Reading the leading element as `xs.first` states the intent directly and is
//! easier to read than the numeric `xs[0]`, which forces the reader to recognize
//! that zero means "first". The rule matches an index expression whose subscript
//! is the integer literal `0`; any other index, including a non-literal
//! expression that evaluates to zero, is left alone. Matching is syntactic on the
//! literal, so it does not confirm the receiver exposes a `first` getter.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferFirst;

impl Rule for PreferFirst {
    fn name(&self) -> &'static str {
        "prefer-first"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    if let Some(body) = &func.body {
                        check_body(body, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Class(class) => {
                    for member in &class.members {
                        check_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Mixin(mixin) => {
                    for member in &mixin.members {
                        check_member(member, &mut diags, ctx);
                    }
                }
                _ => {}
            }
        }
        diags
    }
}

fn check_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    };
    if let Some(body) = body {
        check_body(body, diags, ctx);
    }
}

fn check_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(block) => check_stmts(&block.stmts, diags, ctx),
        FunctionBody::Arrow(expr, _) => check_expr(expr, diags, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn check_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts {
        check_stmt(stmt, diags, ctx);
    }
}

fn check_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Expr(expr_stmt) => check_expr(&expr_stmt.expr, diags, ctx),
        Stmt::Return(ret) => {
            if let Some(expr) = &ret.value {
                check_expr(expr, diags, ctx);
            }
        }
        Stmt::LocalVar(local) => {
            for decl in &local.declarators {
                if let Some(init) = &decl.initializer {
                    check_expr(init, diags, ctx);
                }
            }
        }
        Stmt::Block(block) => check_stmts(&block.stmts, diags, ctx),
        Stmt::If(if_stmt) => {
            check_stmt(&if_stmt.then_branch, diags, ctx);
            if let Some(else_b) = &if_stmt.else_branch {
                check_stmt(else_b, diags, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            if let Some(ForInit::VarDecl(local)) = &for_stmt.init {
                for decl in &local.declarators {
                    if let Some(init) = &decl.initializer {
                        check_expr(init, diags, ctx);
                    }
                }
            }
            check_stmt(&for_stmt.body, diags, ctx);
        }
        Stmt::While(s) => check_stmt(&s.body, diags, ctx),
        Stmt::DoWhile(s) => check_stmt(&s.body, diags, ctx),
        Stmt::TryCatch(s) => {
            check_stmts(&s.body.stmts, diags, ctx);
            for catch in &s.catches {
                check_stmts(&catch.body.stmts, diags, ctx);
            }
        }
        _ => {}
    }
}

fn check_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Index {
            object,
            index,
            span,
            ..
        } => {
            if is_zero(index) {
                diags.push(Diagnostic::new(
                    "prefer-first",
                    Severity::Warning,
                    "Prefer .first over [0] to access the first element",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
            check_expr(object, diags, ctx);
            check_expr(index, diags, ctx);
        }
        Expr::Field { object, .. } => check_expr(object, diags, ctx),
        Expr::Call { callee, args, .. } => {
            check_expr(callee, diags, ctx);
            for arg in &args.positional {
                check_expr(arg, diags, ctx);
            }
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, diags, ctx);
            check_expr(right, diags, ctx);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            check_expr(condition, diags, ctx);
            check_expr(then_expr, diags, ctx);
            check_expr(else_expr, diags, ctx);
        }
        Expr::Assign { target, value, .. } => {
            check_expr(target, diags, ctx);
            check_expr(value, diags, ctx);
        }
        Expr::Await { expr: inner, .. } => check_expr(inner, diags, ctx),
        _ => {}
    }
}

fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::IntLit { value, .. } if value == "0")
}
