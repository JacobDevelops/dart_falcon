//! Flags declared identifiers that are shorter or longer than the allowed range.
//!
//! Very short names rarely convey meaning and very long ones hurt readability, so
//! this rule keeps declared names within a configurable length band. Its scope is
//! deliberately limited to variable and field declarations, getter and setter
//! names, and enum constants; parameters, catch-clause variables, for-each loop
//! variables, and plain function and method names are never checked, because
//! short names are often idiomatic there. A single leading underscore is stripped
//! before both the length and exception checks, so `_id` is judged as `id`.
//!
//! ## Options
//!
//! `min_length` (int, default: 3) — flag identifiers shorter than this.
//! `max_length` (int, default: 300) — flag identifiers longer than this.
//! `exceptions` (list of strings, default: []) — names always allowed regardless of length.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferCorrectIdentifierLength;

/// Resolved options for one analysis pass. dcl defaults: min 3, max 300, no
/// exceptions. There is deliberately no built-in exception list — dcl has none,
/// and the scope below never reaches short-lived names like loop or lambda
/// parameters where single letters are idiomatic.
struct IdentCfg {
    min_length: usize,
    max_length: usize,
    exceptions: HashSet<String>,
}

fn ident_cfg(ctx: &AnalyzeContext) -> IdentCfg {
    let opts = crate::meta::meta_for("prefer-correct-identifier-length")
        .and_then(|m| ctx.rule_options(m.group, "prefer-correct-identifier-length"));

    let min_length = opts
        .and_then(|o| o.get("min_length"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(3);

    let max_length = opts
        .and_then(|o| o.get("max_length"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(300);

    let exceptions: HashSet<String> = opts
        .and_then(|o| o.get("exceptions"))
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    IdentCfg {
        min_length,
        max_length,
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
                // dcl checks *variable declarations*, so a function's own name
                // and parameters are out of scope; only its local variables are.
                TopLevelDecl::Function(func) => {
                    if let Some(body) = &func.body {
                        check_body(body, &mut diags, ctx, &cfg);
                    }
                }
                TopLevelDecl::Variable(var) => {
                    for decl in &var.declarators {
                        check_ident(&decl.name, &mut diags, ctx, &cfg);
                    }
                }
                TopLevelDecl::Class(class) => check_members(&class.members, &mut diags, ctx, &cfg),
                TopLevelDecl::Mixin(mixin) => check_members(&mixin.members, &mut diags, ctx, &cfg),
                TopLevelDecl::MixinClass(mc) => check_members(&mc.members, &mut diags, ctx, &cfg),
                TopLevelDecl::Extension(ext) => check_members(&ext.members, &mut diags, ctx, &cfg),
                TopLevelDecl::Enum(e) => {
                    // Enum constants are checked as declarations…
                    for variant in &e.variants {
                        check_ident(&variant.name, &mut diags, ctx, &cfg);
                    }
                    // …and any getters/setters/fields the enum declares.
                    check_members(&e.members, &mut diags, ctx, &cfg);
                }
                _ => {}
            }
        }
        diags
    }
}

/// dcl strips a single leading underscore before both the exception check and
/// the length check, so `_id` matches the `id` exception and `_ab` is judged as
/// the two-character `ab`.
fn is_invalid(name: &str, cfg: &IdentCfg) -> bool {
    let stripped = name.strip_prefix('_').unwrap_or(name);
    if cfg.exceptions.contains(stripped) {
        return false;
    }
    let len = stripped.chars().count();
    len < cfg.min_length || len > cfg.max_length
}

fn check_ident(
    ident: &Identifier,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &IdentCfg,
) {
    if is_invalid(&ident.name, cfg) {
        diags.push(make_diag(ctx, &ident.span, &ident.name, cfg));
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
            // Fields are variable declarations.
            ClassMember::Field(field) => {
                for decl in &field.declarators {
                    check_ident(&decl.name, diags, ctx, cfg);
                }
            }
            // Only getters and setters have their *name* checked (dcl only
            // inspects accessor declarations, not plain methods).
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
            // Methods and constructors: names and parameters are out of scope,
            // but their bodies still declare checkable local variables.
            ClassMember::Method(method) => {
                if let Some(body) = &method.body {
                    check_body(body, diags, ctx, cfg);
                }
            }
            ClassMember::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
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
            // C-style `for (var i = 0; …)` declares a variable, so its counter
            // is checked. A for-*each* loop variable is a `DeclaredIdentifier`,
            // not a `VariableDeclaration`, and dcl does not check it.
            if let Some(ForInit::VarDecl(local)) = &for_stmt.init {
                for decl in &local.declarators {
                    check_ident(&decl.name, diags, ctx, cfg);
                }
            }
            check_stmt(&for_stmt.body, diags, ctx, cfg);
        }
        Stmt::While(s) => check_stmt(&s.body, diags, ctx, cfg),
        Stmt::DoWhile(s) => check_stmt(&s.body, diags, ctx, cfg),
        Stmt::TryCatch(s) => {
            // A catch clause parameter (`catch (e)`) is not a variable
            // declaration, so only the block bodies are traversed.
            check_stmts(&s.body.stmts, diags, ctx, cfg);
            for catch in &s.catches {
                check_stmts(&catch.body.stmts, diags, ctx, cfg);
            }
            if let Some(fin) = &s.finally {
                check_stmts(&fin.stmts, diags, ctx, cfg);
            }
        }
        Stmt::LocalFunc(local_func) => {
            // The local function's name and parameters are out of scope; only
            // its body's variable declarations matter.
            check_body(&local_func.body, diags, ctx, cfg);
        }
        _ => {}
    }
}

fn make_diag(ctx: &AnalyzeContext, span: &Span, name: &str, cfg: &IdentCfg) -> Diagnostic {
    let len = name.strip_prefix('_').unwrap_or(name).chars().count();
    let message = if len > cfg.max_length {
        format!(
            "The {name} identifier is {len} characters long. It's recommended to decrease it to {} chars long.",
            cfg.max_length
        )
    } else {
        format!(
            "The {name} identifier is {len} characters long. It's recommended to increase it up to {} chars long.",
            cfg.min_length
        )
    };
    Diagnostic::new(
        "prefer-correct-identifier-length",
        Severity::Warning,
        message,
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    )
}
