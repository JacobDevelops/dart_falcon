//! Flags Padding directly wrapping a Flutter Container that can own the padding.
//! Containers with existing padding, margin, alignment, sizing, constraints,
//! transforms, color, decoration or foregroundDecoration are excluded: moving
//! padding across those layout and paint stages can change the result. Non-null
//! keys on either widget are excluded because the keyed element's bounds change.
//! This is distinct from prefer-padding-over-container and prefer-container's
//! three-widget layout sequence.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidWrappingInPadding;

impl Rule for AvoidWrappingInPadding {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "Padding")
            || args.named().any(|arg| {
                arg.name().name() == "key"
                    && !matches!(arg.value().unparenthesized(), Expr::NullLit(_))
            })
        {
            return;
        }
        let matches = {
            args.named()
                .find(|arg| arg.name().name() == "child")
                .and_then(|arg| {
                    crate::lint::flutter::scoped_constructor_invocation(arg.value(), state)
                })
                .is_some_and(|(child, constructor, child_args)| {
                    constructor == "new"
                        && index.is_package_declaration(&child, "flutter", "Container")
                        && !child_args.named().any(|arg| {
                            (arg.name().name() == "key"
                                && !matches!(arg.value().unparenthesized(), Expr::NullLit(_)))
                                || [
                                    "padding",
                                    "margin",
                                    "alignment",
                                    "width",
                                    "height",
                                    "constraints",
                                    "transform",
                                    "color",
                                    "decoration",
                                    "foregroundDecoration",
                                ]
                                .contains(&arg.name().name())
                        })
                })
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic("Move this padding into the child `Container` instead of wrapping it in `Padding`.", crate::lint::flutter::constructor_span(node)));
        }
    }
}
