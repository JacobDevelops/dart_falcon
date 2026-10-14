//! Flags a function or method longer than the configured line limit.
//!
//! Long functions are hard to reason about and test; extracting helpers keeps
//! each unit focused. The rule measures a function's span from its declaration
//! through the end of its body (in source lines) and reports at the declaration
//! when the length exceeds the threshold. Operator bodies are included, while
//! abstract/external declarations and nested functions or closures are not measured.
//!
//! ## Options
//!
//! `max_lines` (integer, default: 100) — flag functions/methods spanning more
//! than this many lines.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_declared_body;

#[derive(Default)]
pub struct MaxLinesForFunction;

impl Rule for MaxLinesForFunction {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for_each_declared_body(program, ctx.source, &mut |_, _, span| {
            check_function_lines(span, &mut diags, ctx);
        });
        diags
    }
}

use crate::lint::expressions::line_of;

/// Read the `max_lines` option (default 100). Malformed/missing → default.
fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "max-lines-for-function"),
        "max_lines",
    )
    .unwrap_or(100)
}

fn check_function_lines(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let start_line = line_of(ctx.source, span.start);
    let end_line = line_of(ctx.source, span.end);
    let lines = end_line - start_line + 1;
    let threshold = max_lines_option(ctx);

    if lines > threshold {
        diags.push(ctx.diagnostic(
            "max-lines-for-function",
            format!("Function exceeds the maximum number of lines ({threshold})."),
            DiagSpan {
                start: span.start,
                end: span.start,
            },
        ));
    }
}
