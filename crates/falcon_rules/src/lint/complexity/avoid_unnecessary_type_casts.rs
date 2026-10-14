//! Flags an `as` cast to the operand's resolved static type.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperationKind, collect};

#[derive(Default)]
pub struct AvoidUnnecessaryTypeCasts;

impl Rule for AvoidUnnecessaryTypeCasts {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(analysis) = collect(program, ctx) else {
            return Vec::new();
        };
        analysis
            .operations
            .into_iter()
            .filter_map(|operation| {
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
            })
            .collect()
    }
}
