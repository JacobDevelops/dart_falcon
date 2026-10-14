//! Flags the null-assertion operator `!` except on a resolved `Map` index.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidNonNullAssertion;

impl Rule for AvoidNonNullAssertion {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::NullAssert { operand, span } = node else {
            return;
        };
        if let Expr::Index { object, .. } = operand.as_ref() {
            let receiver = state.infer(object);
            if state
                .signatures
                .instantiated_supertype(&receiver, "dart:core", "Map", &state.model)
                .is_some()
            {
                return;
            }
            if !known_non_map(&receiver) {
                return;
            }
        }
        self.diags.push(self.ctx.diagnostic(
            "avoid-non-null-assertion",
            "Avoid using the null assertion operator '!'",
            *span,
        ));
    }
}

fn known_non_map(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Null
            | ResolvedType::Void
            | ResolvedType::Never
            | ResolvedType::Function { .. }
            | ResolvedType::Record { .. }
            | ResolvedType::Interface {
                identity: falcon_analyze::DeclarationIdentity::Sdk { .. },
                ..
            }
    )
}
