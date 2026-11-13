//! Flags an `is` check proven impossible by canonical resolved types.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, RuntimeIntersection};
use falcon_syntax::cst::Program;

use crate::lint::semantic_type_operations::{TypeOperation, TypeOperationKind, TypeOperations};
use falcon_analyze::SemanticState;

#[derive(Default)]
pub struct AvoidUnrelatedTypeAssertions;

impl Rule for AvoidUnrelatedTypeAssertions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(TypeOperations {
            diagnostics: Vec::new(),
            report: move |operation: TypeOperation, state: &SemanticState<'_>| {
                if !matches!(operation.kind, TypeOperationKind::Is { negated: false })
                    || state.signatures.runtime_intersection(&operation.operand, &operation.target, &state.model) != RuntimeIntersection::Disjoint
                {
                    return None;
                }
                Some(ctx.diagnostic("Remove this type assertion because the types are unrelated and it can never be true.", operation.span))
            },
        }))
    }
}
