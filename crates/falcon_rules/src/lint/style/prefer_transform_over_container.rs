//! Prefers `Transform` over a `Container` used only for `transform`.
//!
//! The unnamed Flutter constructor must have a provably non-null child and
//! a provably non-null `transform` argument, with only an optional `key`.
//! `transformAlignment` is also allowed and maps to `Transform.alignment`.
//! Other arguments, nullable or unresolved values, and shadowed constructors
//! are ignored. The dedicated widget makes the layout intent explicit.
//! Contexts explicitly requiring `Container` are excluded.

use falcon_analyze::SemanticState;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

use crate::lint::support::widget_replacements::{only_property, prepare};

#[derive(Default)]
pub struct PreferTransformOverContainer;

impl Rule for PreferTransformOverContainer {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(
            ctx,
            "Container",
            "Prefer `Transform` over a `Container` used only for transform.",
            matches,
        )
    }
}

fn matches(args: ArgList<'_>, state: &SemanticState<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    only_property(
        args,
        "transform",
        &["key", "child", "transform", "transformAlignment"],
        state,
        ctx,
    )
}
