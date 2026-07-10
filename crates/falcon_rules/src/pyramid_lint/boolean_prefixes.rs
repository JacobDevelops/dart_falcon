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
        let prefixes = resolve_prefixes(ctx);
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx, &prefixes);
        }
        diags
    }
}

/// Built-in accepted prefixes. A user-provided `prefixes` option EXTENDS this
/// list (pyramid_lint convention) rather than replacing it.
const BUILTIN_PREFIXES: &[&str] = &["is", "has", "can", "should", "was"];

const MESSAGE: &str =
    "Boolean variables should have a prefix like 'is', 'has', 'can', 'should', or 'was'.";

/// Resolve the accepted prefix list: built-ins plus any from the `prefixes`
/// option. Malformed/missing option → built-ins only.
fn resolve_prefixes(ctx: &AnalyzeContext) -> Vec<String> {
    let mut prefixes: Vec<String> = BUILTIN_PREFIXES.iter().map(|s| s.to_string()).collect();
    if let Some(list) = crate::meta::meta_for("boolean_prefixes")
        .and_then(|m| ctx.config.rule_options(m.group, "boolean_prefixes"))
        .and_then(|o| o.get("prefixes"))
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

/// `@override` members inherit their name from the supertype, so the author
/// cannot rename them to satisfy the prefix rule.
fn is_override(annotations: &[Annotation]) -> bool {
    annotations
        .iter()
        .any(|a| a.name.last().is_some_and(|id| id.name == "override"))
}

fn has_valid_boolean_prefix(name: &str, prefixes: &[String]) -> bool {
    // Private members carry a leading underscore that is not part of the
    // conceptual name (`_isEnabled` still "starts with" `is`), so strip it
    // before matching — otherwise every private boolean is a false positive.
    let name = name.trim_start_matches('_');
    prefixes.iter().any(|prefix| {
        name.strip_prefix(prefix.as_str())
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
    prefixes: &[String],
) {
    if is_bool_type(ty) && !has_valid_boolean_prefix(&name.name, prefixes) {
        flag(&name.span, diags, ctx);
    }
}

fn check_params(
    params: &FormalParamList,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    prefixes: &[String],
) {
    for param in params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
    {
        check_named(param.param_type.as_ref(), &param.name, diags, ctx, prefixes);
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
            check_params(&f.params, diags, ctx, prefixes);
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
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                check_named(v.var_type.as_ref(), &d.name, diags, ctx, prefixes);
            }
        }
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
        ClassMember::Field(f) => {
            if !is_override(&f.annotations) {
                for d in &f.declarators {
                    check_named(f.field_type.as_ref(), &d.name, diags, ctx, prefixes);
                }
            }
        }
        ClassMember::Method(m) => {
            if !is_override(&m.annotations) {
                check_named(m.return_type.as_ref(), &m.name, diags, ctx, prefixes);
                check_params(&m.params, diags, ctx, prefixes);
            }
            if let Some(body) = &m.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        ClassMember::Constructor(c) => {
            check_params(&c.params, diags, ctx, prefixes);
            if let Some(body) = &c.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        ClassMember::Getter(g) => {
            if let Some(body) = &g.body {
                scan_body(body, diags, ctx, prefixes);
            }
        }
        ClassMember::Setter(s) => {
            if !is_override(&s.annotations) {
                check_named(s.param_type.as_ref(), &s.param, diags, ctx, prefixes);
            }
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
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                check_named(lv.var_type.as_ref(), &d.name, diags, ctx, prefixes);
            }
        }
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
        Stmt::LocalFunc(lf) => {
            check_params(&lf.params, diags, ctx, prefixes);
            scan_body(&lf.body, diags, ctx, prefixes);
        }
        _ => {}
    }
}
