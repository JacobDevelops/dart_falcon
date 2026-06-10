use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferCorrectIdentifierLength;

const ALLOWED_SINGLE_CHARS: &[&str] = &["i", "j", "k", "n", "_"];

impl Rule for PreferCorrectIdentifierLength {
    fn name(&self) -> &'static str {
        "prefer-correct-identifier-length"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    check_ident(&func.name, &mut diags, ctx);
                    check_params(&func.params, &mut diags, ctx);
                    if let Some(body) = &func.body {
                        check_body(body, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Class(class) => check_members(&class.members, &mut diags, ctx),
                TopLevelDecl::Mixin(mixin) => check_members(&mixin.members, &mut diags, ctx),
                TopLevelDecl::MixinClass(mc) => check_members(&mc.members, &mut diags, ctx),
                TopLevelDecl::Enum(e) => check_members(&e.members, &mut diags, ctx),
                TopLevelDecl::Extension(ext) => check_members(&ext.members, &mut diags, ctx),
                _ => {}
            }
        }
        diags
    }
}

fn is_short(name: &str) -> bool {
    name.len() == 1 && !ALLOWED_SINGLE_CHARS.contains(&name)
}

fn check_ident(ident: &Identifier, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if is_short(&ident.name) {
        diags.push(make_diag(ctx, &ident.span, &ident.name));
    }
}

fn check_params(params: &FormalParamList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for param in params.positional.iter()
        .chain(params.optional_positional.iter())
        .chain(params.named.iter())
    {
        if is_short(&param.name.name) {
            diags.push(make_diag(ctx, &param.name.span, &param.name.name));
        }
    }
}

fn check_members(members: &[ClassMember], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in members {
        match member {
            ClassMember::Field(field) => {
                for decl in &field.declarators {
                    check_ident(&decl.name, diags, ctx);
                }
            }
            ClassMember::Method(method) => {
                check_ident(&method.name, diags, ctx);
                check_params(&method.params, diags, ctx);
                if let Some(body) = &method.body {
                    check_body(body, diags, ctx);
                }
            }
            ClassMember::Constructor(ctor) => {
                check_params(&ctor.params, diags, ctx);
                if let Some(body) = &ctor.body {
                    check_body(body, diags, ctx);
                }
            }
            ClassMember::Getter(getter) => {
                check_ident(&getter.name, diags, ctx);
                if let Some(body) = &getter.body {
                    check_body(body, diags, ctx);
                }
            }
            ClassMember::Setter(setter) => {
                check_ident(&setter.name, diags, ctx);
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
        Stmt::LocalVar(local) => {
            for decl in &local.declarators {
                check_ident(&decl.name, diags, ctx);
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
            match &for_stmt.init {
                Some(ForInit::VarDecl(local)) => {
                    for decl in &local.declarators {
                        check_ident(&decl.name, diags, ctx);
                    }
                }
                Some(ForInit::ForIn { name, .. }) => {
                    check_ident(name, diags, ctx);
                }
                _ => {}
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
            if let Some(fin) = &s.finally {
                check_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(local_func) => {
            check_ident(&local_func.name, diags, ctx);
            check_params(&local_func.params, diags, ctx);
            check_body(&local_func.body, diags, ctx);
        }
        _ => {}
    }
}

fn make_diag(ctx: &AnalyzeContext, span: &Span, name: &str) -> Diagnostic {
    Diagnostic::new(
        "prefer-correct-identifier-length",
        Severity::Warning,
        format!("Identifier '{}' is too short — use a more descriptive name", name),
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan { start: span.start, end: span.end },
    )
}
