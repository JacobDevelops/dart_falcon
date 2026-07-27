//! Await only values proven to be future-like.

use falcon_analyze::{AnalyzeContext, Rule, TypeTruth};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
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
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector {
    diagnostics: Vec<Diagnostic>,
    file: String,
}

impl SemanticRuleVisitor for Collector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Await { expr, span } = node else {
            return;
        };
        if matches!(expr.as_ref(), Expr::NullLit { .. }) {
            return;
        }
        let ty = state.infer(expr);
        if state.model.is_future_like(&ty) == TypeTruth::No {
            self.diagnostics.push(Diagnostic::new(
                "await-only-futures",
                Severity::Warning,
                "Await only futures or other future-like values.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}
