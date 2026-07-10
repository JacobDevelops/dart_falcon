use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct MaxParametersForFunction;

impl Rule for MaxParametersForFunction {
    fn name(&self) -> &'static str {
        "max_parameters_for_function"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn count_parameters(params: &FormalParamList) -> usize {
    params.positional.len() + params.optional_positional.len() + params.named.len()
}

/// Read the `max_parameters` option (default 5). Malformed/missing → default.
fn max_parameters_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("max_parameters_for_function")
        .and_then(|m| {
            ctx.config
                .rule_options(m.group, "max_parameters_for_function")
        })
        .and_then(|o| o.get("max_parameters"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(5)
}

fn check_function_params(
    params: &FormalParamList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let total = count_parameters(params);
    let threshold = max_parameters_option(ctx);
    if total > threshold {
        diags.push(Diagnostic::new(
            "max_parameters_for_function",
            Severity::Warning,
            format!("Function has too many parameters (max {threshold})."),
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            check_function_params(&f.params, &f.span, diags, ctx);
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
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Method(m) => {
            check_function_params(&m.params, &m.span, diags, ctx);
        }
        ClassMember::Constructor(c) => {
            check_function_params(&c.params, &c.span, diags, ctx);
        }
        ClassMember::Operator(op) => {
            check_function_params(&op.params, &op.span, diags, ctx);
        }
        _ => {}
    }
}
