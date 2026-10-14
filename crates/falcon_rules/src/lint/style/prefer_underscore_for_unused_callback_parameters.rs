//! Flags unused closure parameters that are not named with underscores.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_expr_in_program;

use crate::lint::lexical_usage::used_parameters;

#[derive(Default)]
pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for_each_expr_in_program(program, |node| {
            if let Expr::FuncExpr {
                params, body, span, ..
            } = node
            {
                let used = used_parameters(params, body);
                if params.params.iter().any(|param| {
                    let name = &param.name.name;
                    !(name.is_empty()
                        || name.bytes().all(|byte| byte == b'_')
                        || used.contains(&param.name.span.start))
                }) {
                    diagnostics.push(ctx.diagnostic(
                        "prefer-underscore-for-unused-callback-parameters",
                        "Unused callback parameter should be named '_'.",
                        *span,
                    ));
                }
            }
        });
        diagnostics
    }
}
