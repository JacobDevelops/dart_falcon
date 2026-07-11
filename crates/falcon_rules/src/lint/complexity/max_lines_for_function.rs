//! Flags functions longer than the configured line limit. Ported from pyramid_lint's `max_lines_for_function`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct MaxLinesForFunction;

impl Rule for MaxLinesForFunction {
    fn name(&self) -> &'static str {
        "max_lines_for_function"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn line_of(source: &str, offset: usize) -> usize {
    let c = offset.min(source.len());
    source[..c].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Read the `max_lines` option (default 100). Malformed/missing → default.
fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("max_lines_for_function")
        .and_then(|m| ctx.rule_options(m.group, "max_lines_for_function"))
        .and_then(|o| o.get("max_lines"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(100)
}

fn check_function_lines(
    span: &Span,
    has_body: bool,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if !has_body {
        return;
    }

    let start_line = line_of(ctx.source, span.start);
    let end_line = line_of(ctx.source, span.end);
    let lines = end_line - start_line + 1;
    let threshold = max_lines_option(ctx);

    if lines > threshold {
        diags.push(Diagnostic::new(
            "max_lines_for_function",
            Severity::Warning,
            format!("Function exceeds the maximum number of lines ({threshold})."),
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.start,
            },
        ));
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            check_function_lines(&f.span, f.body.is_some(), diags, ctx);
        }
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Enum(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Extension(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Method(m) => {
            check_function_lines(&m.span, m.body.is_some(), diags, ctx);
        }
        ClassMember::Constructor(c) => {
            check_function_lines(&c.span, c.body.is_some(), diags, ctx);
        }
        ClassMember::Getter(g) => {
            check_function_lines(&g.span, g.body.is_some(), diags, ctx);
        }
        ClassMember::Setter(s) => {
            check_function_lines(&s.span, s.body.is_some(), diags, ctx);
        }
        _ => {}
    }
}
