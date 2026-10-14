//! Flags unused closure parameters that are not named with underscores.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::lexical_usage::used_parameters;

#[derive(Default)]
pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
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
                self.diagnostics.push(self.ctx.diagnostic(
                    "prefer-underscore-for-unused-callback-parameters",
                    "Unused callback parameter should be named '_'.",
                    *span,
                ));
            }
        }
        walk_expr(self, node);
    }
}
