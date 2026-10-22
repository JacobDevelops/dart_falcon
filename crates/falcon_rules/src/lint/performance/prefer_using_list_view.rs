//! Suggests considering ListView for a SingleChildScrollView containing a plain
//! Flutter Column with at least two unconditional children in a list literal.
//! Spreads, control-flow elements and dynamic lists do not establish that minimum.
//! This is not a layout-neutral replacement: preserve the
//! Column's child alignment and scroll view's padding explicitly. ListView
//! stretches children across the cross axis and adds MediaQuery padding by default.
//! For the default Column alignment, wrap each list child in an Align with
//! Alignment.topCenter; use the original scroll padding, or EdgeInsets.zero
//! when none was supplied. Review constraints and child lifecycle separately.
//! No automatic fix is offered.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferUsingListView;

impl Rule for PreferUsingListView {
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
        if !index.is_package_declaration(&identity, "flutter", "SingleChildScrollView") {
            return;
        }
        if args
            .named()
            .any(|arg| arg.name().name() == "scrollDirection")
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
                        && index.is_package_declaration(&child, "flutter", "Column")
                        && child_args.positional().is_empty()
                        && child_args
                            .named()
                            .all(|arg| arg.name().name() == "children")
                        && child_args
                            .named()
                            .find(|arg| arg.name().name() == "children")
                            .is_some_and(|arg| {
                                let Expr::List(children) = arg.value().unparenthesized() else {
                                    return false;
                                };
                                children
                                    .elements()
                                    .filter(|element| matches!(element, CollectionElement::Expr(_)))
                                    .nth(1)
                                    .is_some()
                            })
                })
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic("Consider `ListView` for this scrollable `Column`; preserve child alignment and padding explicitly, including `ListView`'s default `MediaQuery` padding.", crate::lint::flutter::constructor_span(node)));
        }
    }
}
