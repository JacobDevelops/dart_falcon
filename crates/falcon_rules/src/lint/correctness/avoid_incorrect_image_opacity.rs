//! Flags Opacity directly wrapping a Flutter Image, including named constructors.
//! Prefer Image.opacity (or image color blending) to compositing the whole widget.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidIncorrectImageOpacity;

impl Rule for AvoidIncorrectImageOpacity {
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
        if !index.is_package_declaration(&identity, "flutter", "Opacity") {
            return;
        }
        let matches = {
            args.named()
                .find(|arg| arg.name().name() == "child")
                .and_then(|arg| {
                    crate::lint::flutter::scoped_constructor_invocation(arg.value(), state)
                })
                .is_some_and(|(child, _, _)| {
                    index.is_package_declaration(&child, "flutter", "Image")
                })
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic("Apply opacity directly to the Image instead of wrapping it in Opacity.", crate::lint::flutter::constructor_span(node)));
        }
    }
}
