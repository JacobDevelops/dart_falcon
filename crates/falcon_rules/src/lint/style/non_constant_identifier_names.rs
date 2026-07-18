//! Flags non-constant identifiers that are not written in lowerCamelCase.
//!
//! Dart convention reserves lowerCamelCase for everything that holds a runtime
//! value — non-const variables and fields, formal and closure parameters,
//! for-in loop variables, function and method names, and named constructors.
//! Consistent casing lets readers distinguish these from types (UpperCamelCase)
//! and compile-time constants at a glance. The check mirrors the analyzer's
//! `isLowerCamelCase`: leading underscores are ignored, an all-underscore
//! wildcard name is accepted, a single uppercase letter is tolerated, and the
//! remainder must begin with a lowercase letter or `$` and contain no further
//! underscores. Type names and constants are covered by separate rules and are
//! out of scope here.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NonConstantIdentifierNames;

impl Rule for NonConstantIdentifierNames {
    fn name(&self) -> &'static str {
        "non-constant-identifier-names"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str = "Name non-constant identifiers using lowerCamelCase.";

/// lowerCamelCase per the analyzer's `isLowerCamelCase`: an all-underscore name
/// (a wildcard) is allowed, leading underscores are ignored, then the remainder
/// must start with a lowercase letter or `$` and contain no further underscores.
/// A single uppercase letter is also accepted (mirrors the analyzer helper).
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
            "non-constant-identifier-names",
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

fn check_params(params: &FormalParamList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for p in params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
    {
        check(&p.name, diags, ctx);
        if let Some(def) = &p.default_value {
            scan_expr(def, diags, ctx);
        }
    }
}

fn check_declarators(
    declarators: &[VarDeclarator],
    is_const: bool,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if is_const {
        return;
    }
    for d in declarators {
        check(&d.name, diags, ctx);
        if let Some(init) = &d.initializer {
            scan_expr(init, diags, ctx);
        }
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if !f.is_getter && !f.is_setter {
                check(&f.name, diags, ctx);
            }
            check_params(&f.params, diags, ctx);
            scan_opt_body(&f.body, diags, ctx);
        }
        TopLevelDecl::Variable(v) => check_declarators(&v.declarators, v.is_const, diags, ctx),
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Enum(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Extension(ext) => ext.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::ExtensionType(x) => x.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => check_declarators(&f.declarators, f.is_const, diags, ctx),
        ClassMember::Method(m) => {
            check(&m.name, diags, ctx);
            check_params(&m.params, diags, ctx);
            scan_opt_body(&m.body, diags, ctx);
        }
        ClassMember::Constructor(c) => {
            if let Some(named) = &c.constructor_name {
                check(named, diags, ctx);
            }
            check_params(&c.params, diags, ctx);
            scan_opt_body(&c.body, diags, ctx);
        }
        ClassMember::Getter(g) => scan_opt_body(&g.body, diags, ctx),
        ClassMember::Setter(s) => {
            check(&s.param, diags, ctx);
            scan_opt_body(&s.body, diags, ctx);
        }
        _ => {}
    }
}

fn scan_opt_body(body: &Option<FunctionBody>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(b) = body {
        scan_body(b, diags, ctx);
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx),
        FunctionBody::Native(..) => {}
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        scan_stmt(s, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::LocalVar(lv) => check_declarators(&lv.declarators, lv.is_const, diags, ctx),
        Stmt::LocalFunc(lf) => {
            check(&lf.name, diags, ctx);
            check_params(&lf.params, diags, ctx);
            scan_body(&lf.body, diags, ctx);
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
        Stmt::For(f) => {
            if let Some(ForInit::ForIn { name, .. }) = &f.init {
                check(name, diags, ctx);
            }
            scan_stmt(&f.body, diags, ctx);
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::Switch(sw) => {
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
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx);
            }
        }
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
        _ => {}
    }
}

/// Only descends into expressions that can host a closure (`FuncExpr`), whose
/// parameters are also non-constant identifiers.
fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::FuncExpr { params, body, .. } => {
            check_params(params, diags, ctx);
            scan_body(body, diags, ctx);
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx);
            for a in &args.positional {
                scan_expr(a, diags, ctx);
            }
            for a in &args.named {
                scan_expr(&a.value, diags, ctx);
            }
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Assign { value, .. } => scan_expr(value, diags, ctx),
        Expr::Await { expr, .. } | Expr::Throw { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::Conditional {
            then_expr,
            else_expr,
            ..
        } => {
            scan_expr(then_expr, diags, ctx);
            scan_expr(else_expr, diags, ctx);
        }
        _ => {}
    }
}
