//! Flags an `as` cast to the operand's resolved static type.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperationKind, collect};

pub struct AvoidUnnecessaryTypeCasts;

impl Rule for AvoidUnnecessaryTypeCasts {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-casts"
    }

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
                Some(Diagnostic::new(
                    self.name(),
                    Severity::Warning,
                    "Unnecessary type cast — variable is already known to be this type",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: operation.span.start,
                        end: operation.span.end,
                    },
                ))
            })
            .collect()
    }
}
