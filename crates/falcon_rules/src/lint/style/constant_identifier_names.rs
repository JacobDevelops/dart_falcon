//! Flags constant identifiers that are not lowerCamelCase.
//!
//! Dart names constants in `lowerCamelCase`, not the `SCREAMING_CAPS` common in
//! other languages, so `maxCount` rather than `MAX_COUNT`. Following the
//! convention keeps constants visually consistent with every other identifier.
//! The rule covers `const` declarations at every scope — top-level, static and
//! instance fields, and locals — as well as enum values. All-underscore
//! wildcard names are permitted.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct ConstantIdentifierNames;

impl Rule for ConstantIdentifierNames {
    fn name(&self) -> &'static str {
        "constant-identifier-names"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str = "Name constant identifiers using lowerCamelCase.";

/// lowerCamelCase per the analyzer's `isLowerCamelCase`: an all-underscore name
/// (a wildcard) is allowed, leading underscores are ignored, then the remainder
/// must start with a lowercase letter or `$` and contain no further underscores.
fn is_lower_camel_case(name: &str) -> bool {
    if !name.is_empty() && name.bytes().all(|b| b == b'_') {
        return true;
    }
    if name.len() == 1 && name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return true;
    }
    let rest = name.trim_start_matches('_');
    let Some(first) = rest.chars().next() else {
        return true;
    };
    if !(first.is_ascii_lowercase() || first == '$') {
        return false;
    }
    rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '$')
}

fn check(name: &Identifier, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !is_lower_camel_case(&name.name) {
        diags.push(Diagnostic::new(
            "constant-identifier-names",
            Severity::Warning,
            MESSAGE,
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: name.span.start,
                end: name.span.end,
            },
        ));
    }
}

fn check_const_declarators(
    declarators: &[VarDeclarator],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    for d in declarators {
        check(&d.name, diags, ctx);
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Variable(v) if v.is_const => {
            check_const_declarators(&v.declarators, diags, ctx);
        }
        TopLevelDecl::Function(f) => scan_opt_body(&f.body, diags, ctx),
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Extension(ext) => ext.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::ExtensionType(x) => x.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Enum(e) => {
            for variant in &e.variants {
                check(&variant.name, diags, ctx);
            }
            e.members.iter().for_each(|m| scan_member(m, diags, ctx));
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) if f.is_const => {
            check_const_declarators(&f.declarators, diags, ctx);
        }
        ClassMember::Method(m) => scan_opt_body(&m.body, diags, ctx),
        ClassMember::Constructor(c) => scan_opt_body(&c.body, diags, ctx),
        ClassMember::Getter(g) => scan_opt_body(&g.body, diags, ctx),
        ClassMember::Setter(s) => scan_opt_body(&s.body, diags, ctx),
        _ => {}
    }
}

fn scan_opt_body(body: &Option<FunctionBody>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(FunctionBody::Block(b)) = body {
        scan_stmts(&b.stmts, diags, ctx);
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        scan_stmt(s, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::LocalVar(lv) if lv.is_const => check_const_declarators(&lv.declarators, diags, ctx),
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => {
            if let FunctionBody::Block(b) = &lf.body {
                scan_stmts(&b.stmts, diags, ctx);
            }
        }
        _ => {}
    }
}
