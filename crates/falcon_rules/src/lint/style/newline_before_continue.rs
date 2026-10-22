//! Requires a blank line before a continue that follows another statement.
//!
//! Checks labeled and unlabeled continues in blocks. First statements, unbraced
//! continues, and switch-case lists are exempt. Blank lines above comments count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::support::newline_spacing::{self, Target};

#[derive(Default)]
pub struct NewlineBeforeContinue;

impl Rule for NewlineBeforeContinue {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        newline_spacing::prepare(ctx, Target::Continue)
    }
}
