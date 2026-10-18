//! Requires a blank line before constructors following another member.
//!
//! Includes named, redirecting, and factory constructor declarations.
//! The first member is exempt unless enum constants precede it; constructor
//! invocations are not checked. Blank lines above attached comments count.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

use super::newline_spacing::{self, Target};

#[derive(Default)]
pub struct NewlineBeforeConstructor;

impl Rule for NewlineBeforeConstructor {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        newline_spacing::prepare(ctx, Target::Constructor)
    }
}
