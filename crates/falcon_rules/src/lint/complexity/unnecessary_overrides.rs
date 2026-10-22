//! Flags a member that does nothing but forward to `super` with
//! the same arguments and resolved contract.
//!
//! Such an override adds visual noise and a maintenance burden without changing
//! behavior — delete it and the inherited implementation stays in force.
//! Resolved inherited metadata and signatures preserve meaningful overrides,
//! including documentation, body comments, narrower contracts, and visibility changes.
//! Ordinary Flutter State forwarding overrides belong here; only overrides that
//! drop inherited `@protected` belong to `avoid-unnecessary-overrides-in-state`.

use falcon_analyze::{
    AnalyzeContext, Rule,
    RulePreparation,
};

use falcon_syntax::cst::Program;

#[derive(Default)]
pub struct UnnecessaryOverrides;

impl Rule for UnnecessaryOverrides {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(ctx, false)
    }
}

use crate::lint::support::forwarding_overrides::prepare;
