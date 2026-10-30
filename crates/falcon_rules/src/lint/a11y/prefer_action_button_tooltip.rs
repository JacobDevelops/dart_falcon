//! Requires useful tooltips on Flutter floating action buttons and icon buttons.
//!
//! Tooltips describe actions to assistive technology. Resolved Flutter
//! `FloatingActionButton` (unnamed, small, large, extended) and `IconButton` (unnamed,
//! filled, filledTonal, outlined) constructors are checked. Missing, literal null,
//! and empty non-interpolated string tooltips are reported. Dynamic labels and
//! other button APIs are not guessed; custom classes and shadowed names are ignored.
//! This checks the button's own tooltip argument, including extended FABs;
//! labels or enclosing Tooltip/Semantics widgets do not exempt the button.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferActionButtonTooltip;

impl Rule for PreferActionButtonTooltip {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if !matches!(node, Expr::Call(_) | Expr::New(_)) {
            return;
        }
        let Some((identity, constructor, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        let supported =
            (index.is_package_declaration(&identity, "flutter", "FloatingActionButton")
                && matches!(constructor, "new" | "small" | "large" | "extended"))
                || (index.is_package_declaration(&identity, "flutter", "IconButton")
                    && matches!(constructor, "new" | "filled" | "filledTonal" | "outlined"));
        if !supported {
            return;
        }
        let tooltip = args
            .named()
            .find(|argument| argument.name().name() == "tooltip");
        let missing = tooltip.is_none_or(|argument| match argument.value().unparenthesized() {
            Expr::NullLit(_) => true,
            Expr::StringLit(string) => {
                string.interpolations().next().is_none()
                    && string.segments().next().is_some()
                    && string
                        .segments()
                        .all(|(delimiter, _)| delimiter.content.is_empty())
            }
            _ => false,
        });
        if missing {
            self.diags.push(self.ctx.diagnostic("Provide a non-empty tooltip describing this action button.", tooltip.map_or_else(
                || crate::lint::flutter::constructor_span(node),
                |argument| argument.value().span(),
            )));
        }
    }
}
