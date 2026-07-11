//! Flags comparisons against boolean literals. Ported from dart_code_linter's `no-boolean-literal-compare`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoBooleanLiteralCompare;

impl Rule for NoBooleanLiteralCompare {
    fn name(&self) -> &'static str {
        "no-boolean-literal-compare"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// An expression whose result is *provably* a boolean without type resolution:
/// a literal, a negation, an `is`/`is!` check, or a comparison/logical binary.
/// Identifiers, member accesses and calls are excluded — their nullability is
/// unknowable, and `x == true` is the correct null-safe form for a `bool?`.
fn is_known_bool(expr: &Expr) -> bool {
    match expr {
        Expr::BoolLit { .. } => true,
        Expr::Unary {
            op: UnaryOp::Bang, ..
        } => true,
        Expr::Is { .. } => true,
        Expr::Binary { op, .. } => matches!(
            op,
            BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::LtEq
                | BinaryOp::GtEq
                | BinaryOp::And
                | BinaryOp::Or
        ),
        _ => false,
    }
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
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
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
        FunctionBody::Block(b) => scan_stmts(&b.stmts, diags, ctx),
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
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            if let IfCondition::Expr(e) = &i.condition {
                scan_expr(e, diags, ctx);
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
            if let Some(ForInit::VarDecl(lv)) = &f.init {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, ctx);
            }
            for u in &f.update {
                scan_expr(u, diags, ctx);
            }
            scan_stmt(&f.body, diags, ctx);
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
        Stmt::Assert(a) => {
            scan_expr(&a.condition, diags, ctx);
            if let Some(msg) = &a.message {
                scan_expr(msg, diags, ctx);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) {
                let left_bool = matches!(left.as_ref(), Expr::BoolLit { .. });
                let right_bool = matches!(right.as_ref(), Expr::BoolLit { .. });
                // Flag only when the *other* operand is a provably boolean
                // expression. Without type resolution, an identifier or member
                // access compared to a literal (`x == true`) is unknowable: it
                // is the null-safe idiom for a `bool?`, which dcl exempts because
                // it checks the operand's non-nullable-bool static type. See the
                // meta note demoting this rule from the recommended preset.
                let other = if left_bool {
                    Some(right.as_ref())
                } else if right_bool {
                    Some(left.as_ref())
                } else {
                    None
                };
                if other.is_some_and(is_known_bool) {
                    diags.push(Diagnostic::new(
                        "no-boolean-literal-compare",
                        Severity::Warning,
                        "Avoid comparing boolean values to boolean literals",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
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
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
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
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
        _ => {}
    }
}
