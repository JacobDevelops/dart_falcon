//! Requires a blank line before method declarations following another member.
//!
//! The first member is exempt unless enum constants precede it. Checks methods
//! in classes, mixins, enums, and extensions; top-level and local functions are
//! not method declarations. Blank lines above attached comments count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use super::newline_spacing::{self, Target};

#[derive(Default)]
pub struct NewlineBeforeMethod;

impl Rule for NewlineBeforeMethod {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        newline_spacing::prepare(ctx, Target::Method)
    }
}
