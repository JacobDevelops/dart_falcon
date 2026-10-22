//! Requires a blank line between successive switch statement case clauses.
//!
//! The first clause is exempt, and grouped labels need no blank line between
//! them. Nested switches are checked independently, including default clauses.
//! Blank lines above attached comments count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use crate::lint::support::newline_spacing::{self, Target};

#[derive(Default)]
pub struct NewlineBeforeCase;

impl Rule for NewlineBeforeCase {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        newline_spacing::prepare(ctx, Target::Case)
    }
}
