//! Flags `!iterable.where(...).isEmpty` and `iterable.where(...).length == other.length`.
//!
//! Both patterns ask whether every element satisfies a predicate;
//! `iterable.every(...)` says so directly and short-circuits on the first
//! failure. The rule matches a negated `.where(...).isEmpty` and a
//! `.where(...).length` compared for equality against a `.length`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferIterableEvery;

impl Rule for PreferIterableEvery {
    fn name(&self) -> &'static str {
        "prefer-iterable-every"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// Check if expression matches pattern: !.where(...).isEmpty
fn is_negated_where_is_empty(expr: &Expr) -> Option<Span> {
    if let Expr::Unary {
        op: UnaryOp::Bang,
        operand,
        span,
    } = expr
        && let Expr::Field { object, field, .. } = &**operand
        && field.name == "isEmpty"
        && let Expr::Call { callee, .. } = &**object
        && let Expr::Field {
            field: where_field, ..
        } = &**callee
        && where_field.name == "where"
    {
        return Some(Span {
            start: span.start,
            end: span.end,
        });
    }
    None
}

/// Check if expression matches pattern: .where(...).length == .length
fn is_where_length_eq_length(expr: &Expr) -> Option<Span> {
    if let Expr::Binary {
        op: BinaryOp::EqEq,
        left,
        right,
        span,
    } = expr
    {
        // Check if left is something.where(...).length
        if let Expr::Field {
            object: left_obj,
            field: left_field,
            ..
        } = &**left
            && left_field.name == "length"
            && let Expr::Call {
                callee: left_callee,
                ..
            } = &**left_obj
            && let Expr::Field {
                field: left_where_field,
                object: _left_where_object,
                ..
            } = &**left_callee
            && left_where_field.name == "where"
        {
            // Check if right is iterable.length (the original iterable)
            if let Expr::Field {
                field: right_field, ..
            } = &**right
                && right_field.name == "length"
            {
                return Some(Span {
                    start: span.start,
                    end: span.end,
                });
            }
        }
    }
    None
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "prefer-iterable-every",
        Severity::Warning,
        "Use .every() instead of .where().isEmpty or .where().length comparison.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx);
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
        _ => {}
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
    if let Some(b) = body {
        scan_body(b, diags, ctx);
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => {
            scan_stmts(&b.stmts, diags, ctx);
        }
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        scan_stmt(s, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(b) => {
            scan_stmts(&b.stmts, diags, ctx);
        }
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        Stmt::If(i) => {
            match &i.condition {
                IfCondition::Expr(e) => scan_expr(e, diags, ctx),
                IfCondition::Case(e, _, guard) => {
                    scan_expr(e, diags, ctx);
                    if let Some(g) = guard {
                        scan_expr(g, diags, ctx);
                    }
                }
            }
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => {
            scan_expr(&w.condition, diags, ctx);
            scan_stmt(&w.body, diags, ctx);
        }
        Stmt::DoWhile(d) => {
            scan_stmt(&d.body, diags, ctx);
            scan_expr(&d.condition, diags, ctx);
        }
        Stmt::For(f) => {
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, ctx);
            }
            scan_stmt(&f.body, diags, ctx);
        }
        Stmt::Switch(sw) => {
            scan_expr(&sw.subject, diags, ctx);
            for case in &sw.cases {
                scan_stmts(&case.body, diags, ctx);
            }
        }
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(span) = is_negated_where_is_empty(expr) {
        flag(&span, diags, ctx);
    }

    if let Some(span) = is_where_length_eq_length(expr) {
        flag(&span, diags, ctx);
    }

    match expr {
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
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
        _ => {}
    }
}
