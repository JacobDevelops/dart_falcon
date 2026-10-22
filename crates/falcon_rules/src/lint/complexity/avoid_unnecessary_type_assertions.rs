//! Flags an `is` check whose result is already guaranteed by the resolved static type.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::semantic_type_operations::{TypeOperation, TypeOperationKind, TypeOperations};
use falcon_analyze::semantic_scope::SemanticState;

#[derive(Default)]
pub struct AvoidUnnecessaryTypeAssertions;

fn same_type_or_raw_target(operand: &ResolvedType, target: &ResolvedType) -> bool {
    if operand == target {
        return true;
    }
    matches!(
        (operand, target),
        (
            ResolvedType::Interface { identity: left, .. },
            ResolvedType::Interface { identity: right, arguments, .. }
        ) if left == right && arguments.is_empty()
    )
}

impl Rule for AvoidUnnecessaryTypeAssertions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(TypeOperations {
            diagnostics: Vec::new(),
            report: move |operation: TypeOperation, _state: &SemanticState<'_>| {
                if !matches!(operation.kind, TypeOperationKind::Is { negated: false })
                    || !same_type_or_raw_target(&operation.operand, &operation.target)
                    || operation.operand.nullable()
                    || matches!(
                        operation.operand,
                        ResolvedType::Unknown | ResolvedType::Dynamic
                    )
                {
                    return None;
                }
                Some(ctx.diagnostic("Remove this type assertion because the variable is already known to have this type.", operation.span))
            },
        }))
    }
}
