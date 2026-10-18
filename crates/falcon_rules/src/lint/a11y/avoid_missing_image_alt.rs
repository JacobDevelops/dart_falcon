//! Requires semanticLabel on Flutter Image and imageSemanticLabel on FadeInImage.
//! Explicitly null labels are missing; empty labels and dynamic label expressions
//! are accepted. Explicit true or dynamic excludeFromSemantics values are exempt;
//! only an absent flag or literal false proves an image needs a label.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMissingImageAlt;

impl Rule for AvoidMissingImageAlt {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
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
        let Some((identity, _, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities else {
            return;
        };
        let label = if index.is_package_declaration(&identity, "flutter", "Image") {
            "semanticLabel"
        } else if index.is_package_declaration(&identity, "flutter", "FadeInImage") {
            "imageSemanticLabel"
        } else {
            return;
        };
        let matches = {
            !args.named().any(|arg| arg.name().name() == label
            && !matches!(arg.value().unparenthesized(), Expr::NullLit(_)))
            && !args.named().any(|arg| arg.name().name() == "excludeFromSemantics"
                && !matches!(arg.value().unparenthesized(), Expr::BoolLit(value) if !value.value()))
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-missing-image-alt", "Provide a semantic label for this image, or explicitly exclude decorative images from semantics.",
                crate::lint::flutter::constructor_span(node),
            ));
        }
    }
}
