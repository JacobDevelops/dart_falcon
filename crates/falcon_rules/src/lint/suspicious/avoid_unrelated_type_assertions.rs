//! Flags an `is` check proven impossible by canonical resolved types.

use falcon_analyze::{AnalyzeContext, Rule, TypeTruth};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperationKind, collect};

pub struct AvoidUnrelatedTypeAssertions;

impl Rule for AvoidUnrelatedTypeAssertions {
    fn name(&self) -> &'static str {
        "avoid-unrelated-type-assertions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(analysis) = collect(program, ctx) else {
            return Vec::new();
        };
        analysis
            .operations
            .iter()
            .filter_map(|operation| {
                if !matches!(operation.kind, TypeOperationKind::Is { negated: false })
                    || analysis.signatures.unrelated(
                        &operation.operand,
                        &operation.target,
                        &analysis.model,
                    ) != TypeTruth::Yes
                {
                    return None;
                }
                Some(Diagnostic::new(
                    self.name(),
                    Severity::Warning,
                    "Type assertion can never be true — types are unrelated",
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
