//! Prefers `Center` over an `Align` whose alignment is known to be centered.
//!
//! Reports Flutter's unnamed `Align` constructor with omitted alignment,
//! `Alignment.center`, or a Flutter `Alignment(0, 0)` constructor. `key`,
//! `child`, `widthFactor`, and `heightFactor` are preserved by `Center` and
//! remain allowed, including an absent child. Other arguments, null or unknown
//! alignments, and shadowed constructors or alignment names are ignored.

use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, expression_segments};
use falcon_syntax::cst::*;

use crate::lint::support::widget_replacements::{argument, only_named, prepare};

#[derive(Default)]
pub struct PreferCenterOverAlign;

impl Rule for PreferCenterOverAlign {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(
            ctx,
            "Align",
            "Prefer `Center` over an `Align` with centered alignment.",
            matches,
        )
    }
}

fn matches(args: ArgList<'_>, state: &SemanticState<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    only_named(
        args,
        &["key", "child", "alignment", "widthFactor", "heightFactor"],
    ) && argument(args, "alignment").is_none_or(|value| centered(value, state, ctx))
}

fn centered(value: Expr<'_>, state: &SemanticState<'_>, ctx: &AnalyzeContext<'_>) -> bool {
    if let Expr::Field(field) = value.unparenthesized()
        && !field.is_null_safe()
        && field.field().name() == "center"
        && let Some(segments) = expression_segments(field.object(), false)
        && let Some(head) = segments.first()
        && !crate::lint::flutter::value_name_is_shadowed(head, state)
        && let Some(identity) = state.model.resolve_dotted(&segments)
        && ctx.identities().is_some_and(|index| {
            index.is_package_declaration(&state.model.unaliased(identity), "flutter", "Alignment")
        })
    {
        return true;
    }
    if let Some((identity, "new", args)) =
        crate::lint::flutter::scoped_constructor_invocation(value, state)
        && ctx
            .identities()
            .is_some_and(|index| index.is_package_declaration(&identity, "flutter", "Alignment"))
        && args.named().next().is_none()
    {
        let mut positional = args.positional();
        return positional.next().is_some_and(zero)
            && positional.next().is_some_and(zero)
            && positional.next().is_none();
    }
    false
}

fn zero(value: Expr<'_>) -> bool {
    match value.unparenthesized() {
        Expr::IntLit(literal) => literal.value().parse::<i64>() == Ok(0),
        Expr::DoubleLit(literal) => literal.value().parse::<f64>() == Ok(0.0),
        _ => false,
    }
}
