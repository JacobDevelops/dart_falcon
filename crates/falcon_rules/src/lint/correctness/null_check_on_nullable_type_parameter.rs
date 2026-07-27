//! Avoid null assertions that merely coerce a nullable type parameter to itself.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct NullCheckOnNullableTypeParameter;

impl Rule for NullCheckOnNullableTypeParameter {
    fn name(&self) -> &'static str {
        "null-check-on-nullable-type-parameter"
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
            "null-check-on-nullable-type-parameter",
            Severity::Warning,
            "Avoid a null assertion on a nullable type parameter.",
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
        let Expr::NullAssert { operand, span } = node else {
            return;
        };
        let operand_type = state.infer(operand);
        if let ResolvedType::TypeParameter {
            id, nullable: true, ..
        } = &operand_type
            && matches!(state.expected(), Some(ResolvedType::TypeParameter { id: expected_id, nullable: false, .. }) if expected_id == id)
        {
            self.report(span);
        }
    }

    fn visit_pattern(
        &mut self,
        node: &Pattern,
        matched: Option<&ResolvedType>,
        _state: &SemanticState<'_>,
    ) {
        if let Pattern::NullAssert { span, .. } = node
            && matches!(
                matched,
                Some(ResolvedType::TypeParameter { nullable: true, .. })
            )
        {
            self.report(span);
        }
    }
}
