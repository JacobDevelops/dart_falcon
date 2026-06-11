use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UnnecessaryNullableReturnType;

impl Rule for UnnecessaryNullableReturnType {
    fn name(&self) -> &'static str {
        "unnecessary_nullable_return_type"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(return_type) = &f.return_type
                && is_nullable_type(return_type)
                && let Some(body) = &f.body
                && !body_can_return_null(body)
            {
                flag_return_type(return_type, diags, ctx);
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
    match member {
        ClassMember::Method(m) => {
            if let Some(return_type) = &m.return_type
                && is_nullable_type(return_type)
                && let Some(body) = &m.body
                && !body_can_return_null(body)
            {
                flag_return_type(return_type, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            if let Some(body) = &g.body
                && !body_can_return_null(body)
            {
                // Getter return type must be inferred from the return value or body expression
                // For now, we'll skip getters since they don't have explicit return types
            }
        }
        _ => {}
    }
}

/// Check if a type is nullable (ends with `?` or has a nullable type argument)
fn is_nullable_type(ty: &DartType) -> bool {
    match ty {
        DartType::Named(nt) => {
            // Check if the type itself is nullable
            if nt.is_nullable {
                return true;
            }
            // Check if any type argument is nullable
            for arg in &nt.type_args {
                if is_nullable_type(arg) {
                    return true;
                }
            }
            false
        }
        DartType::Function(ft) => ft.is_nullable,
        DartType::Record(rt) => rt.is_nullable,
        _ => false,
    }
}

/// Check if a function body can return null
fn body_can_return_null(body: &FunctionBody) -> bool {
    match body {
        FunctionBody::Block(b) => stmts_can_return_null(&b.stmts),
        FunctionBody::Arrow(e, _) => expr_is_null(e),
        FunctionBody::Native(_, _) => false,
    }
}

/// Check if a list of statements can return null
fn stmts_can_return_null(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Return(ret) => {
                if let Some(v) = &ret.value {
                    if expr_is_null(v) {
                        return true;
                    }
                } else {
                    // Explicit `return;` without value is implicitly null
                    return true;
                }
            }
            Stmt::Block(b) => {
                if stmts_can_return_null(&b.stmts) {
                    return true;
                }
            }
            Stmt::If(i) => {
                if stmts_can_return_null_from_if(i) {
                    return true;
                }
            }
            Stmt::TryCatch(tc) => {
                if stmts_can_return_null(&tc.body.stmts) {
                    return true;
                }
                for catch in &tc.catches {
                    if stmts_can_return_null(&catch.body.stmts) {
                        return true;
                    }
                }
                if let Some(fin) = &tc.finally
                    && stmts_can_return_null(&fin.stmts)
                {
                    return true;
                }
            }
            Stmt::While(w) => {
                if stmts_can_return_null_from_stmt(&w.body) {
                    return true;
                }
            }
            Stmt::DoWhile(d) => {
                if stmts_can_return_null_from_stmt(&d.body) {
                    return true;
                }
            }
            Stmt::For(f) => {
                if stmts_can_return_null_from_stmt(&f.body) {
                    return true;
                }
            }
            Stmt::Switch(s) if switch_can_return_null(s) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn stmts_can_return_null_from_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Block(b) => stmts_can_return_null(&b.stmts),
        Stmt::Return(ret) => {
            if let Some(v) = &ret.value {
                expr_is_null(v)
            } else {
                true
            }
        }
        Stmt::If(i) => stmts_can_return_null_from_if(i),
        _ => false,
    }
}

fn stmts_can_return_null_from_if(i: &IfStmt) -> bool {
    if stmts_can_return_null_from_stmt(&i.then_branch) {
        return true;
    }
    if let Some(else_branch) = &i.else_branch {
        stmts_can_return_null_from_stmt(else_branch)
    } else {
        false
    }
}

fn switch_can_return_null(s: &SwitchStmt) -> bool {
    for case in &s.cases {
        if stmts_can_return_null(&case.body) {
            return true;
        }
    }
    false
}

fn expr_is_null(expr: &Expr) -> bool {
    matches!(expr, Expr::NullLit { .. })
}

fn flag_return_type(ty: &DartType, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let span = ty.span();
    diags.push(Diagnostic::new(
        "unnecessary_nullable_return_type",
        Severity::Warning,
        "Function return type is unnecessarily nullable",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
