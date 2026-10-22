//! Requires a blank line before a throw that follows another statement.
//!
//! Checks blocks, including nested bodies and closures. First statements,
//! unbraced throws, and switch-case lists are exempt. Blank lines above attached
//! comments count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::support::newline_spacing::{self, Target};

#[derive(Default)]
pub struct NewlineBeforeThrow;

impl Rule for NewlineBeforeThrow {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        newline_spacing::prepare(ctx, Target::Throw)
    }
}
