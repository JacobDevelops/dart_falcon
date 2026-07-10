use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferCorrectIdentifierLength;

/// Built-in names always allowed regardless of length. User-provided
/// `exceptions` extend (not replace) this list.
const BUILTIN_EXCEPTIONS: &[&str] = &["i", "j", "k", "n", "_"];

/// Resolved options for one analysis pass.
struct IdentCfg {
    /// Names strictly shorter than this are flagged (default 2 → single chars).
    min_length: usize,
    exceptions: HashSet<String>,
}

fn ident_cfg(ctx: &AnalyzeContext) -> IdentCfg {
    let opts = crate::meta::meta_for("prefer-correct-identifier-length").and_then(|m| {
        ctx.config
            .rule_options(m.group, "prefer-correct-identifier-length")
    });

    let min_length = opts
        .and_then(|o| o.get("min_length"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(2);

    let mut exceptions: HashSet<String> =
        BUILTIN_EXCEPTIONS.iter().map(|s| s.to_string()).collect();
    if let Some(list) = opts
        .and_then(|o| o.get("exceptions"))
        .and_then(|v| v.as_array())
    {
        for name in list.iter().filter_map(|v| v.as_str()) {
            exceptions.insert(name.to_string());
        }
    }

    IdentCfg {
        min_length,
        exceptions,
    }
}

impl Rule for PreferCorrectIdentifierLength {
    fn name(&self) -> &'static str {
        "prefer-correct-identifier-length"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let cfg = ident_cfg(ctx);
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    check_ident(&func.name, &mut diags, ctx, &cfg);
                    check_params(&func.params, &mut diags, ctx, &cfg);
                    if let Some(body) = &func.body {
                        check_body(body, &mut diags, ctx, &cfg);
                    }
                }
                TopLevelDecl::Class(class) => check_members(&class.members, &mut diags, ctx, &cfg),
                TopLevelDecl::Mixin(mixin) => check_members(&mixin.members, &mut diags, ctx, &cfg),
                TopLevelDecl::MixinClass(mc) => check_members(&mc.members, &mut diags, ctx, &cfg),
                TopLevelDecl::Enum(e) => check_members(&e.members, &mut diags, ctx, &cfg),
                TopLevelDecl::Extension(ext) => check_members(&ext.members, &mut diags, ctx, &cfg),
                _ => {}
            }
        }
        diags
    }
}

fn is_short(name: &str, cfg: &IdentCfg) -> bool {
    name.chars().count() < cfg.min_length && !cfg.exceptions.contains(name)
}

fn check_ident(
    ident: &Identifier,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &IdentCfg,
) {
    if is_short(&ident.name, cfg) {
        diags.push(make_diag(ctx, &ident.span, &ident.name));
    }
}

fn check_params(
    params: &FormalParamList,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &IdentCfg,
) {
    for param in params
        .positional
        .iter()
        .chain(params.optional_positional.iter())
        .chain(params.named.iter())
    {
        if is_short(&param.name.name, cfg) {
            diags.push(make_diag(ctx, &param.name.span, &param.name.name));
        }
    }
}

fn check_members(
    members: &[ClassMember],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &IdentCfg,
) {
    for member in members {
        match member {
            ClassMember::Field(field) => {
                for decl in &field.declarators {
                    check_ident(&decl.name, diags, ctx, cfg);
                }
            }
            ClassMember::Method(method) => {
                check_ident(&method.name, diags, ctx, cfg);
                check_params(&method.params, diags, ctx, cfg);
                if let Some(body) = &method.body {
                    check_body(body, diags, ctx, cfg);
                }
            }
            ClassMember::Constructor(ctor) => {
                check_params(&ctor.params, diags, ctx, cfg);
                if let Some(body) = &ctor.body {
                    check_body(body, diags, ctx, cfg);
                }
            }
            ClassMember::Getter(getter) => {
                check_ident(&getter.name, diags, ctx, cfg);
                if let Some(body) = &getter.body {
                    check_body(body, diags, ctx, cfg);
                }
            }
            ClassMember::Setter(setter) => {
                check_ident(&setter.name, diags, ctx, cfg);
                if let Some(body) = &setter.body {
                    check_body(body, diags, ctx, cfg);
                }
            }
            _ => {}
        }
    }
}

fn check_body(
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &IdentCfg,
) {
    match body {
        FunctionBody::Block(block) => check_stmts(&block.stmts, diags, ctx, cfg),
        FunctionBody::Arrow(_, _) | FunctionBody::Native(_, _) => {}
    }
}

fn check_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &IdentCfg) {
    for stmt in stmts {
        check_stmt(stmt, diags, ctx, cfg);
    }
}

fn check_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &IdentCfg) {
    match stmt {
        Stmt::LocalVar(local) => {
            for decl in &local.declarators {
                check_ident(&decl.name, diags, ctx, cfg);
            }
        }
        Stmt::Block(block) => check_stmts(&block.stmts, diags, ctx, cfg),
        Stmt::If(if_stmt) => {
            check_stmt(&if_stmt.then_branch, diags, ctx, cfg);
            if let Some(else_b) = &if_stmt.else_branch {
                check_stmt(else_b, diags, ctx, cfg);
            }
        }
        Stmt::For(for_stmt) => {
            match &for_stmt.init {
                Some(ForInit::VarDecl(local)) => {
                    for decl in &local.declarators {
                        check_ident(&decl.name, diags, ctx, cfg);
                    }
                }
                Some(ForInit::ForIn { name, .. }) => {
                    check_ident(name, diags, ctx, cfg);
                }
                _ => {}
            }
            check_stmt(&for_stmt.body, diags, ctx, cfg);
        }
        Stmt::While(s) => check_stmt(&s.body, diags, ctx, cfg),
        Stmt::DoWhile(s) => check_stmt(&s.body, diags, ctx, cfg),
        Stmt::TryCatch(s) => {
            check_stmts(&s.body.stmts, diags, ctx, cfg);
            for catch in &s.catches {
                check_stmts(&catch.body.stmts, diags, ctx, cfg);
            }
            if let Some(fin) = &s.finally {
                check_stmts(&fin.stmts, diags, ctx, cfg);
            }
        }
        Stmt::LocalFunc(local_func) => {
            check_ident(&local_func.name, diags, ctx, cfg);
            check_params(&local_func.params, diags, ctx, cfg);
            check_body(&local_func.body, diags, ctx, cfg);
        }
        _ => {}
    }
}

fn make_diag(ctx: &AnalyzeContext, span: &Span, name: &str) -> Diagnostic {
    Diagnostic::new(
        "prefer-correct-identifier-length",
        Severity::Warning,
        format!(
            "Identifier '{}' is too short — use a more descriptive name",
            name
        ),
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    )
}
