//! Flags an `is` check whose result is already guaranteed by the resolved static type.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperationKind, collect};

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
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-assertions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(analysis) = collect(program, ctx) else {
            return Vec::new();
        };
        analysis
            .operations
            .into_iter()
            .filter_map(|operation| {
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
                Some(Diagnostic::new(
                    self.name(),
                    Severity::Warning,
                    "Unnecessary type assertion — variable is already known to be this type",
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
