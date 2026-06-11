use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct BooleanPrefixes;

impl Rule for BooleanPrefixes {
    fn name(&self) -> &'static str {
        "boolean_prefixes"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const MESSAGE: &str =
    "Boolean variables should have a prefix like 'is', 'has', 'can', 'should', or 'was'.";

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "boolean_prefixes",
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

fn has_valid_boolean_prefix(name: &str) -> bool {
    const PREFIXES: &[&str] = &["is", "has", "can", "should", "was"];
    PREFIXES.iter().any(|prefix| {
        name.strip_prefix(*prefix)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_uppercase())
    })
}

/// Flag a declared name when its type is `bool` and the name lacks an accepted prefix.
fn check_named(
    ty: Option<&DartType>,
    name: &Identifier,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if is_bool_type(ty) && !has_valid_boolean_prefix(&name.name) {
        flag(&name.span, diags, ctx);
    }
}

fn check_params(params: &FormalParamList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for param in params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
    {
        check_named(param.param_type.as_ref(), &param.name, diags, ctx);
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            check_params(&f.params, diags, ctx);
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx);
            }
        }
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(mc) => mc.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                check_named(v.var_type.as_ref(), &d.name, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => {
            for d in &f.declarators {
                check_named(f.field_type.as_ref(), &d.name, diags, ctx);
            }
        }
        ClassMember::Method(m) => {
            check_named(m.return_type.as_ref(), &m.name, diags, ctx);
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
            if let Some(body) = &g.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            check_named(s.param_type.as_ref(), &s.param, diags, ctx);
            if let Some(body) = &s.body {
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
            for d in &lv.declarators {
                check_named(lv.var_type.as_ref(), &d.name, diags, ctx);
            }
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
        Stmt::LocalFunc(lf) => {
            check_params(&lf.params, diags, ctx);
            scan_body(&lf.body, diags, ctx);
        }
        _ => {}
    }
}
