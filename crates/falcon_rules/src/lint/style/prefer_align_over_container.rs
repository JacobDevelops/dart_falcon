//! Prefers `Align` over a `Container` used only for `alignment`.
//!
//! The unnamed Flutter constructor must have a provably non-null child and
//! a provably non-null `alignment` argument, with only an optional `key`.
//! Other arguments, nullable or unresolved values, and shadowed constructors
//! are ignored. The dedicated widget makes the layout intent explicit.
//! Contexts explicitly requiring `Container` are excluded.

use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

use super::widget_replacements::{only_property, prepare};

#[derive(Default)]
pub struct PreferAlignOverContainer;

impl Rule for PreferAlignOverContainer {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(
            ctx,
            "Container",
            "Prefer 'Align' over a Container used only for alignment.",
            matches,
        )
    }
}

fn matches(args: ArgList<'_>, state: &SemanticState<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    only_property(
        args,
        "alignment",
        &["key", "child", "alignment"],
        state,
        ctx,
    )
}
