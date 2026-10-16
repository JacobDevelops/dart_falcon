//! Flags an `is` check proven impossible by canonical resolved types.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, Truth};
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperation, TypeOperationKind, TypeOperations};
use falcon_analyze::semantic_scope::SemanticState;

#[derive(Default)]
pub struct AvoidUnrelatedTypeAssertions;

impl Rule for AvoidUnrelatedTypeAssertions {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(TypeOperations {
            diagnostics: Vec::new(),
            report: move |operation: TypeOperation, state: &SemanticState<'_>| {
                if !matches!(operation.kind, TypeOperationKind::Is { negated: false })
                    || state.signatures.unrelated(
                        &operation.operand,
                        &operation.target,
                        &state.model,
                    ) != Truth::Yes
                {
                    return None;
                }
                Some(ctx.diagnostic(
                    "avoid-unrelated-type-assertions",
                    "Type assertion can never be true — types are unrelated",
                    operation.span,
                ))
            },
        }))
    }
}
