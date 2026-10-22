//! Requires a blank line before a break that follows another statement.
//!
//! Checks labeled and unlabeled breaks in blocks. First statements, unbraced
//! breaks, and switch-case lists are exempt. Blank lines above comments count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::support::newline_spacing::{self, Target};

#[derive(Default)]
pub struct NewlineBeforeBreak;

impl Rule for NewlineBeforeBreak {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        newline_spacing::prepare(ctx, Target::Break)
    }
}
