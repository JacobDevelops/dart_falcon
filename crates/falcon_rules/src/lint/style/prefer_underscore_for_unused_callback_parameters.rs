//! Flags unused closure parameters that are not named with underscores.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::resolve::lexical_usage::used_parameters;

#[derive(Default)]
pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
            if let Expr::FuncExpr(function) = node.unparenthesized()
            {
                let used = used_parameters(function.params(), function.body());
                if function.params().params().any(|param| {
                    let name = param.name().name();
                    !(name.is_empty()
                        || name.bytes().all(|byte| byte == b'_')
                        || used.contains(&param.name().span().start))
                }) {
                    self.diagnostics.push(self.ctx.diagnostic(
                        "prefer-underscore-for-unused-callback-parameters",
                        "Unused callback parameter should be named '_'.",
                        function.span(),
                    ));
                }
            }
        StructuralTraversal::Continue
    }
}
}
