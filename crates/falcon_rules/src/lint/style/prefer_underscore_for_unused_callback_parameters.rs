//! Flags unused closure parameters that are not named with underscores.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use falcon_analyze::resolve::lexical_usage::used_parameters;

#[derive(Default)]
pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
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
                    self.diagnostics.push(self.ctx.diagnostic(
                        "prefer-underscore-for-unused-callback-parameters",
                        "Unused callback parameter should be named '_'.",
                        *span,
                    ));
                }
            }
        StructuralTraversal::Continue
    }
}
}
