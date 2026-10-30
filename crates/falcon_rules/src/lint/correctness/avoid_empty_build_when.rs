//! Require an explicit `buildWhen` argument on Bloc builders and consumers.
//!
//! Checks resolved `BlocBuilder` and `BlocConsumer` constructor invocations
//! from the Bloc packages. Specifying the argument, even as `null`, satisfies
//! this rule; `BlocSelector` and unrelated or shadowed names are not builders.

use crate::lint::{bloc, flutter};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Expr, Program};

#[derive(Default)]
pub struct AvoidEmptyBuildWhen;

impl Rule for AvoidEmptyBuildWhen {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if !matches!(node, Expr::Call(_) | Expr::New(_)) {
            return;
        }
        let Some((identity, _, args)) = flutter::scoped_constructor_invocation(node, state) else {
            return;
        };
        if (bloc::is_api_identity(&identity, &state.model, "BlocBuilder")
            || bloc::is_api_identity(&identity, &state.model, "BlocConsumer"))
            && !args
                .named()
                .any(|argument| argument.name().name() == "buildWhen")
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Specify `buildWhen` to control when the builder rebuilds.",
                flutter::constructor_span(node),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
