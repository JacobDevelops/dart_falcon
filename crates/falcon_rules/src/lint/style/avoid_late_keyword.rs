//! Flags use of the `late` keyword. Ported from dart_code_linter's `avoid-late-keyword`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidLateKeyword;

impl Rule for AvoidLateKeyword {
    fn name(&self) -> &'static str {
        "avoid-late-keyword"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Variable(var) if var.is_late => {
                    diags.push(make_diag(ctx, &var.span));
                }
                TopLevelDecl::Class(class) => check_members(&class.members, &mut diags, ctx),
                TopLevelDecl::Mixin(mixin) => check_members(&mixin.members, &mut diags, ctx),
                TopLevelDecl::MixinClass(mc) => check_members(&mc.members, &mut diags, ctx),
                TopLevelDecl::Enum(e) => check_members(&e.members, &mut diags, ctx),
                TopLevelDecl::Extension(ext) => check_members(&ext.members, &mut diags, ctx),
                TopLevelDecl::Function(func) => {
                    if let Some(body) = &func.body {
                        check_body(body, &mut diags, ctx);
                    }
                }
                _ => {}
            }
        }
        diags
    }
}

fn check_members(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in members {
        match member {
            ClassMember::Field(field) if field.is_late => diags.push(make_diag(ctx, &field.span)),
            ClassMember::Method(method) => {
                if let Some(body) = &method.body {
                    check_body(body, diags, ctx);
                }
            }
            ClassMember::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    check_body(body, diags, ctx);
                }
            }
            ClassMember::Getter(getter) => {
                if let Some(body) = &getter.body {
                    check_body(body, diags, ctx);
                }
            }
            ClassMember::Setter(setter) => {
                if let Some(body) = &setter.body {
                    check_body(body, diags, ctx);
                }
            }
            _ => {}
        }
    }
}

fn check_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(block) => check_stmts(&block.stmts, diags, ctx),
        FunctionBody::Arrow(_, _) | FunctionBody::Native(_, _) => {}
    }
}

fn check_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts {
        check_stmt(stmt, diags, ctx);
    }
}

fn check_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::LocalVar(local) if local.is_late => diags.push(make_diag(ctx, &local.span)),
        Stmt::Block(block) => check_stmts(&block.stmts, diags, ctx),
        Stmt::If(if_stmt) => {
            check_stmt(&if_stmt.then_branch, diags, ctx);
            if let Some(else_branch) = &if_stmt.else_branch {
                check_stmt(else_branch, diags, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            if let Some(ForInit::VarDecl(local)) = &for_stmt.init
                && local.is_late
            {
                diags.push(make_diag(ctx, &local.span));
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
            if let Some(finally) = &s.finally {
                check_stmts(&finally.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(local_func) => check_body(&local_func.body, diags, ctx),
        _ => {}
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_diag(ctx: &AnalyzeContext, span: &Span) -> Diagnostic {
    Diagnostic::new(
        "avoid-late-keyword",
        Severity::Warning,
        "Avoid using the late keyword — use nullable types or initialize immediately instead",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    )
}
