//! Flags local variables and parameters whose names begin with an underscore.
//!
//! A leading underscore marks a declaration as library-private, but that
//! privacy only applies to top-level and class members — locals, parameters,
//! closure parameters, for-in variables, and catch clause bindings are already
//! confined to their scope. Prefixing them with an underscore implies a privacy
//! that does not exist and misleads the reader, so drop it. Wildcard names made
//! solely of underscores (e.g. `_`, `__`) are exempt, being the idiomatic way
//! to name a deliberately unused binding.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoLeadingUnderscoresForLocalIdentifiers;

impl Rule for NoLeadingUnderscoresForLocalIdentifiers {
    fn name(&self) -> &'static str {
        "no-leading-underscores-for-local-identifiers"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str = "Avoid leading underscores for local identifiers.";

/// A disallowed leading underscore: starts with `_` but is not composed solely
/// of underscores (an all-underscore name is a wildcard).
fn has_leading_underscore(name: &str) -> bool {
    name.starts_with('_') && !name.bytes().all(|b| b == b'_')
}

fn check(name: &Identifier, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if has_leading_underscore(&name.name) {
        diags.push(Diagnostic::new(
            "no-leading-underscores-for-local-identifiers",
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

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            check_params(&f.params, diags, ctx);
            scan_opt_body(&f.body, diags, ctx);
        }
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
        ClassMember::Method(m) => {
            check_params(&m.params, diags, ctx);
            scan_opt_body(&m.body, diags, ctx);
        }
        ClassMember::Constructor(c) => {
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
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                check(&d.name, diags, ctx);
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        Stmt::LocalFunc(lf) => {
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
                if let Some(v) = &catch.exception_var {
                    check(v, diags, ctx);
                }
                if let Some(v) = &catch.stack_trace_var {
                    check(v, diags, ctx);
                }
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

/// Descends only into expressions that can host a closure (`FuncExpr`), whose
/// parameters are also local identifiers.
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
