//! Reject equality checks between types proven to be unrelated.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, TypeTruth};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct UnrelatedTypeEqualityChecks;

impl Rule for UnrelatedTypeEqualityChecks {
    fn name(&self) -> &'static str {
        "unrelated-type-equality-checks"
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

impl Collector {
    fn report(&mut self, span: &Span) {
        self.diagnostics.push(Diagnostic::new(
            "unrelated-type-equality-checks",
            Severity::Warning,
            "Equality checks should compare related types.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl SemanticRuleVisitor for Collector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Binary {
            op: BinaryOp::EqEq | BinaryOp::NotEq,
            left,
            right,
            span,
        } = node
            && !matches!(left.as_ref(), Expr::NullLit { .. })
            && !matches!(right.as_ref(), Expr::NullLit { .. })
        {
            let left_type = state.infer(left);
            let right_type = state.infer(right);
            if state
                .signatures
                .unrelated(&left_type, &right_type, &state.model)
                == TypeTruth::Yes
            {
                self.report(span);
            }
        }
    }

    fn visit_pattern(
        &mut self,
        node: &Pattern,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if let Pattern::Relational {
            op: RelationalPatternOp::Eq | RelationalPatternOp::NotEq,
            value,
            span,
        } = node
            && !matches!(value, Expr::NullLit { .. })
            && let Some(matched) = matched
        {
            let value_type = state.infer(value);
            if state
                .signatures
                .unrelated(matched, &value_type, &state.model)
                == TypeTruth::Yes
            {
                self.report(span);
            }
        }
    }
}
