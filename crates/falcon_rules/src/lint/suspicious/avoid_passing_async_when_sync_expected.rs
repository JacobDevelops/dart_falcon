//! Flags an `async` closure passed to a proven synchronous callback parameter.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct AvoidPassingAsyncWhenSyncExpected;

impl Rule for AvoidPassingAsyncWhenSyncExpected {
    fn name(&self) -> &'static str {
        "avoid-passing-async-when-sync-expected"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diagnostics: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::FuncExpr {
            async_marker: AsyncMarker::Async | AsyncMarker::AsyncStar,
            span,
            ..
        } = node
        else {
            return;
        };
        let Some(ResolvedType::Function { return_type, .. }) = state.expected() else {
            return;
        };
        if state.model.is_future_like(return_type) == Truth::No {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-passing-async-when-sync-expected",
                "Avoid passing an async function where a synchronous callback is expected",
                *span,
            ));
        }
    }
}
