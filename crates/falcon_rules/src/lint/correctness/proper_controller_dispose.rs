//! Requires Flutter controllers owned by a `State` field to be disposed.
//! Shared field checks recognize eagerly evaluated releases, including arguments
//! and operands, but not deferred closures or conditional expression branches.

use crate::lint::support::controller_disposal::{DisposalKind, prepare_disposal};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct ProperControllerDispose;

impl Rule for ProperControllerDispose {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_disposal(ctx, DisposalKind::Controller)
    }
}

