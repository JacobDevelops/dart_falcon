//! Flags an `as` cast to the operand's resolved static type.

use falcon_analyze::{AnalyzeContext, ResolvedTypeView, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::semantic_type_operations::{TypeOperation, TypeOperationKind, TypeOperations};
use falcon_analyze::SemanticState;

#[derive(Default)]
pub struct AvoidUnnecessaryTypeCasts;

impl Rule for AvoidUnnecessaryTypeCasts {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(TypeOperations {
            diagnostics: Vec::new(),
            report: move |operation: TypeOperation, _state: &SemanticState<'_>| {
                if !matches!(operation.kind, TypeOperationKind::As)
                    || operation.operand != operation.target
                    || operation.operand.nullable()
                    || matches!(
                        operation.operand.view(),
                        ResolvedTypeView::Unknown | ResolvedTypeView::Dynamic
                    )
                {
                    return None;
                }
                Some(ctx.diagnostic("Remove this type cast because the variable is already known to have this type.", operation.span))
            },
        }))
    }
}
