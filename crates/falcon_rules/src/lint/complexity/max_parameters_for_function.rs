//! Flags a function, method, or operator with more than the configured number
//! of parameters.
//!
//! A long parameter list is hard to call correctly and usually signals that
//! some arguments belong together in an object. The rule counts all positional,
//! optional, and named parameters. Matching dart_code_linter's
//! number-of-parameters metric, constructors are not counted — so wide
//! named-parameter constructors, the dominant Flutter and DI pattern, are
//! exempt — and `copyWith` methods are skipped.
//!
//! ## Options
//!
//! `max_parameters` (integer, default: 5) — flag when the parameter count
//! exceeds this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct MaxParametersForFunction;

impl Rule for MaxParametersForFunction {
    fn name(&self) -> &'static str {
        "max-parameters-for-function"
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
    crate::meta::meta_for("max-parameters-for-function")
        .and_then(|m| ctx.rule_options(m.group, "max-parameters-for-function"))
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
            "max-parameters-for-function",
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
            // dcl's number-of-parameters metric skips `copyWith` methods that
            // return their own class — the canonical wide-parameter builder.
            if m.name.name == "copyWith" {
                return;
            }
            check_function_params(&m.params, &m.span, diags, ctx);
        }
        // Constructors are `ConstructorDeclaration` nodes, which the dcl metric
        // does not support, so wide named-parameter constructors (the dominant
        // Flutter/DI pattern) are not counted. Operators are method
        // declarations but always have fixed, tiny arity.
        ClassMember::Operator(op) => {
            check_function_params(&op.params, &op.span, diags, ctx);
        }
        _ => {}
    }
}
