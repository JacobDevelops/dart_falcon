use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct AvoidPositionalFieldsInRecords;

impl Rule for AvoidPositionalFieldsInRecords {
    fn name(&self) -> &'static str {
        "avoid_positional_fields_in_records"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str = "Avoid positional fields in records. Use named fields instead.";

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "avoid_positional_fields_in_records",
        Severity::Warning,
        MESSAGE,
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan { start: span.start, end: span.end },
    ));
}

/// Flag any record *type* that declares positional fields, recursing through
/// type arguments and nested record/function types.
fn check_type(ty: Option<&DartType>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let Some(ty) = ty else { return };
    match ty {
        DartType::Record(rec) => {
            if !rec.positional.is_empty() {
                flag(&rec.span, diags, ctx);
            }
            for p in &rec.positional {
                check_type(Some(p), diags, ctx);
            }
            for n in &rec.named {
                check_type(Some(&n.field_type), diags, ctx);
            }
        }
        DartType::Named(named) => {
            for arg in &named.type_args {
                check_type(Some(arg), diags, ctx);
            }
        }
        DartType::Function(f) => {
            if let Some(rt) = &f.return_type {
                check_type(Some(rt), diags, ctx);
            }
            for p in &f.params {
                check_type(Some(&p.param_type), diags, ctx);
            }
        }
        _ => {}
    }
}

fn check_params(params: &FormalParamList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for p in params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
    {
        check_type(p.param_type.as_ref(), diags, ctx);
    }
}

/// True when a record literal carries at least one positional (unnamed) field.
fn record_literal_has_positional(expr: &Expr) -> bool {
    matches!(expr, Expr::Record { fields, .. } if fields.iter().any(|f| f.name.is_none()))
}

/// A record literal is flagged only when it is the initializer of a declaration
/// WITHOUT an explicit record type — when a record type is present it is already
/// flagged at the type, so we avoid double-reporting on the same construct.
fn check_declarators(
    declarators: &[VarDeclarator],
    has_type: bool,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    for d in declarators {
        if let Some(init) = &d.initializer
            && !has_type && record_literal_has_positional(init) {
                flag(init.span(), diags, ctx);
            }
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            check_type(f.return_type.as_ref(), diags, ctx);
            check_params(&f.params, diags, ctx);
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx);
            }
        }
        TopLevelDecl::Variable(v) => {
            check_type(v.var_type.as_ref(), diags, ctx);
            check_declarators(&v.declarators, v.var_type.is_some(), diags, ctx);
        }
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(mc) => mc.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Enum(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Extension(ext) => ext.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => {
            check_type(f.field_type.as_ref(), diags, ctx);
            check_declarators(&f.declarators, f.field_type.is_some(), diags, ctx);
        }
        ClassMember::Method(m) => {
            check_type(m.return_type.as_ref(), diags, ctx);
            check_params(&m.params, diags, ctx);
            if let Some(body) = &m.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Constructor(c) => {
            check_params(&c.params, diags, ctx);
            if let Some(body) = &c.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            check_type(g.return_type.as_ref(), diags, ctx);
            if let Some(body) = &g.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            check_type(s.param_type.as_ref(), diags, ctx);
            if let Some(body) = &s.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Operator(o) => {
            check_type(o.return_type.as_ref(), diags, ctx);
            check_params(&o.params, diags, ctx);
            if let Some(body) = &o.body {
                scan_body(body, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let FunctionBody::Block(b) = body {
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
        Stmt::LocalVar(lv) => {
            check_type(lv.var_type.as_ref(), diags, ctx);
            check_declarators(&lv.declarators, lv.var_type.is_some(), diags, ctx);
        }
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                scan_stmts(&case.body, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}
