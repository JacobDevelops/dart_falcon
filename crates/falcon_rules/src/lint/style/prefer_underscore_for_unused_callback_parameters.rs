//! Flags unused closure parameters that are not named with underscores.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_expr_in_program;

use falcon_analyze::resolve::lexical_usage::used_parameters;

#[derive(Default)]
pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diagnostics = Vec::new();
        for_each_expr_in_program(program, |node| {
            if let Expr::FuncExpr {
                params, body, span, ..
            } = node.unparenthesized()
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
        RulePreparation::Diagnostics(diagnostics)
    }
}
