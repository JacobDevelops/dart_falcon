//! Prefers `ConstrainedBox` over a `Container` used only for `constraints`.
//!
//! The unnamed Flutter constructor must have a provably non-null child and
//! a provably non-null `constraints` argument, with only an optional `key`.
//! Other arguments, nullable or unresolved values, and shadowed constructors
//! are ignored. The dedicated widget makes the layout intent explicit.
//! Contexts explicitly requiring `Container` are excluded.

use falcon_analyze::SemanticState;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

use crate::lint::support::widget_replacements::{only_property, prepare};

#[derive(Default)]
pub struct PreferConstrainedBoxOverContainer;

impl Rule for PreferConstrainedBoxOverContainer {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(
            ctx,
            "Container",
            "Prefer `ConstrainedBox` over a `Container` used only for constraints.",
            matches,
        )
    }
}

fn matches(args: ArgList<'_>, state: &SemanticState<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    only_property(
        args,
        "constraints",
        &["key", "child", "constraints"],
        state,
        ctx,
    )
}
