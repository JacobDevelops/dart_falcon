//! Flags an `is` check proven impossible by canonical resolved types.

use falcon_analyze::{AnalyzeContext, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::Program;

use crate::lint::semantic_type_operations::{TypeOperationKind, collect};

#[derive(Default)]
pub struct AvoidUnrelatedTypeAssertions;

impl Rule for AvoidUnrelatedTypeAssertions {
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
                    ) != Truth::Yes
                {
                    return None;
                }
                Some(ctx.diagnostic(
                    "avoid-unrelated-type-assertions",
                    "Type assertion can never be true — types are unrelated",
                    operation.span,
                ))
            })
            .collect()
    }
}
