//! Flags an `async` closure passed to a proven synchronous callback parameter.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidPassingAsyncWhenSyncExpected;

impl Rule for AvoidPassingAsyncWhenSyncExpected {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::FuncExpr(function) = node else {
            return;
        };
        if !matches!(
            function.async_marker(),
            AsyncMarker::Async | AsyncMarker::AsyncStar
        ) {
            return;
        }
        let Some(ResolvedType::Function { return_type, .. }) = state.expected() else {
            return;
        };
        if state.model.is_future_like(return_type) == Truth::No {
            self.diagnostics.push(self.ctx.diagnostic("Avoid passing an async function where a synchronous callback is expected.", function.span()));
        }
    }
}
