//! Flags an `as` cast to the operand's resolved static type.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperation, TypeOperationKind, TypeOperations};
use falcon_analyze::semantic_scope::SemanticState;

#[derive(Default)]
pub struct AvoidUnnecessaryTypeCasts;

impl Rule for AvoidUnnecessaryTypeCasts {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(TypeOperations {
            diagnostics: Vec::new(),
            report: move |operation: TypeOperation, _state: &SemanticState<'_>| {
                if !matches!(operation.kind, TypeOperationKind::As)
                    || operation.operand != operation.target
                    || operation.operand.nullable()
                    || matches!(
                        operation.operand,
                        ResolvedType::Unknown | ResolvedType::Dynamic
                    )
                {
                    return None;
                }
                Some(ctx.diagnostic(
                    "avoid-unnecessary-type-casts",
                    "Unnecessary type cast — variable is already known to be this type",
                    operation.span,
                ))
            },
        }))
    }
}
