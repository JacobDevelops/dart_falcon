//! Await only values proven to be future-like.

use falcon_analyze::{AnalyzeContext, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct AwaitOnlyFutures;

impl Rule for AwaitOnlyFutures {
    fn name(&self) -> &'static str {
        "await-only-futures"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Await { expr, span } = node else {
            return;
        };
        if matches!(expr.as_ref(), Expr::NullLit { .. }) {
            return;
        }
        let ty = state.infer(expr);
        if state.model.is_future_like(&ty) == Truth::No {
            self.diagnostics.push(self.ctx.diagnostic(
                "await-only-futures",
                "Await only futures or other future-like values.",
                *span,
            ));
        }
    }
}
