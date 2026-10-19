//! Discourage files containing only re-exports, without declarations or parts.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidBarrelFiles;

impl Rule for AvoidBarrelFiles {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let diagnostics = if program.declarations().next().is_none()
            && program.part_directives().next().is_none()
            && program.part_of_directive().is_none()
        {
            program
                .exports()
                .map(|export| {
                    ctx.diagnostic(
                "avoid-barrel-files",
                "Avoid barrel files containing only re-exports; import declarations directly.",
                export.span(),
            )
                })
                .collect()
        } else {
            Vec::new()
        };
        RulePreparation::Diagnostics(diagnostics)
    }
}
