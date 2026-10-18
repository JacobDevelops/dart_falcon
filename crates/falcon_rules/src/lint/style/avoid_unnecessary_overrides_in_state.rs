//! Redundant Flutter State overrides that drop inherited `@protected`.
//!
//! Shares the general override rule's forwarding, signature, documentation and
//! comment checks, but handles only the protected-annotation mismatch that rule
//! deliberately preserves. All other redundant overrides remain owned by
//! `unnecessary-overrides`, so enabling both rules does not duplicate diagnostics.
//! The enclosing type must resolve to a subtype of Flutter's State declaration.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct AvoidUnnecessaryOverridesInState;

impl Rule for AvoidUnnecessaryOverridesInState {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::complexity::unnecessary_overrides::prepare(ctx, true)
    }
}
