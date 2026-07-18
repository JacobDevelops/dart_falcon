//! Flags boolean identifiers that lack a conventional interrogative prefix.
//!
//! A boolean named `visible` reads as a value, whereas `isVisible` reads as the
//! yes/no question it answers; a consistent prefix (`is`, `has`, `can`, ...)
//! makes conditions self-documenting. Only variables and fields initialized
//! with a boolean *literal*, and `bool`-returning methods, getters, and
//! functions, are checked — parameters and uninitialized fields are not. A
//! single leading underscore is stripped before matching, and `@override`
//! members are exempt because they cannot rename an inherited declaration.
//!
//! ## Options
//!
//! `valid_prefixes` (list of strings, default: `["is", "are", "was", "were",
//! "has", "have", "had", "can", "should", "will", "do", "does", "did"]`) —
//! accepted boolean-name prefixes. User-provided entries extend the defaults
//! rather than replacing them.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct BooleanPrefixes;

impl Rule for BooleanPrefixes {
    fn name(&self) -> &'static str {
        "boolean-prefixes"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let prefixes = resolve_prefixes(ctx);
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx, &prefixes);
        }
        diags
    }
}

/// pyramid_lint's default valid prefixes. A user-provided `valid_prefixes`
/// option EXTENDS this list rather than replacing it.
const DEFAULT_PREFIXES: &[&str] = &[
    "is", "are", "was", "were", "has", "have", "had", "can", "should", "will", "do", "does", "did",
];

const MESSAGE: &str = "Boolean should be named with a valid prefix.";

fn resolve_prefixes(ctx: &AnalyzeContext) -> Vec<String> {
    let mut prefixes: Vec<String> = DEFAULT_PREFIXES.iter().map(|s| s.to_string()).collect();
    if let Some(list) = crate::meta::meta_for("boolean-prefixes")
        .and_then(|m| ctx.rule_options(m.group, "boolean-prefixes"))
        .and_then(|o| o.get("valid_prefixes"))
        .and_then(|v| v.as_array())
    {
        for p in list.iter().filter_map(|v| v.as_str()) {
            prefixes.push(p.to_string());
        }
    }
    prefixes
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "boolean-prefixes",
        Severity::Warning,
        MESSAGE,
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

fn is_bool_type(ty: Option<&DartType>) -> bool {
    matches!(ty, Some(DartType::Named(nt)) if nt.segments.len() == 1 && nt.segments[0].name == "bool")
}

fn is_override(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|a| a.name.last().is_some_and(|id| id.name == "override"))
}

/// pyramid_lint strips a single leading underscore and then checks whether the
/// name simply *starts with* a valid prefix (no camelCase boundary is required,
/// mirroring `validPrefixes.any(name.startsWith)`).
fn has_valid_boolean_prefix(name: &str, prefixes: &[String]) -> bool {
    let name = name.strip_prefix('_').unwrap_or(name);
    prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix.as_str()))
}

fn check_name(
    name: &Identifier,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    if !has_valid_boolean_prefix(&name.name, prefixes) {
        flag(&name.span, diags, ctx);
    }
}

/// A variable/field is only inspected when its initializer is a boolean *literal*
/// (`= true` / `= false`). pyramid_lint hooks `BooleanLiteral` nodes whose parent
/// is a `VariableDeclaration`, so an uninitialized `bool` field or one assigned a
/// non-literal expression (`= !kDebugMode`) is out of scope, and the declared
/// type is irrelevant.
fn check_declarators(
    declarators: &[VarDeclarator],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    for d in declarators {
        if matches!(&d.initializer, Some(Expr::BoolLit { .. })) {
            check_name(&d.name, diags, ctx, prefixes);
        }
    }
}

fn scan_top(
    decl: &TopLevelDecl,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    match decl {
        TopLevelDecl::Function(f) => {
            // A top-level function/getter whose return type is `bool`.
            if !f.is_setter && is_bool_type(f.return_type.as_ref()) {
                check_name(&f.name, diags, ctx, prefixes);
            }
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        TopLevelDecl::Class(c) => c
            .members
            .iter()
            .for_each(|m| scan_member(m, diags, ctx, prefixes)),
        TopLevelDecl::Mixin(m) => m
            .members
            .iter()
            .for_each(|m| scan_member(m, diags, ctx, prefixes)),
        TopLevelDecl::MixinClass(mc) => mc
            .members
            .iter()
            .for_each(|m| scan_member(m, diags, ctx, prefixes)),
        TopLevelDecl::Enum(e) => e
            .members
            .iter()
            .for_each(|m| scan_member(m, diags, ctx, prefixes)),
        TopLevelDecl::Extension(ext) => ext
            .members
            .iter()
            .for_each(|m| scan_member(m, diags, ctx, prefixes)),
        TopLevelDecl::Variable(v) => check_declarators(&v.declarators, diags, ctx, prefixes),
        _ => {}
    }
}

fn scan_member(
    member: &ClassMember,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    match member {
        ClassMember::Field(f) => check_declarators(&f.declarators, diags, ctx, prefixes),
        ClassMember::Method(m) => {
            if !is_override(&m.annotations) && is_bool_type(m.return_type.as_ref()) {
                check_name(&m.name, diags, ctx, prefixes);
            }
            if let Some(body) = &m.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        ClassMember::Getter(g) => {
            if !is_override(&g.annotations) && is_bool_type(g.return_type.as_ref()) {
                check_name(&g.name, diags, ctx, prefixes);
            }
            if let Some(body) = &g.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        // Setters and constructors are out of scope; their bodies can still
        // declare boolean-literal locals.
        ClassMember::Constructor(c) => {
            if let Some(body) = &c.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        ClassMember::Setter(s) => {
            if let Some(body) = &s.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        _ => {}
    }
}

fn scan_body(
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    if let FunctionBody::Block(b) = body {
        scan_stmts(&b.stmts, diags, ctx, prefixes);
    }
}

fn scan_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    for s in stmts {
        scan_stmt(s, diags, ctx, prefixes);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, prefixes: &[String]) {
    match stmt {
        Stmt::LocalVar(lv) => check_declarators(&lv.declarators, diags, ctx, prefixes),
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx, prefixes),
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx, prefixes);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx, prefixes);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx, prefixes),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx, prefixes),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx, prefixes),
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx, prefixes);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx, prefixes);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx, prefixes);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx, prefixes),
        _ => {}
    }
}
