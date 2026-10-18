//! Flags a resolved Flutter GestureDetector without any non-null event handlers.
//! Such a detector adds no gestures; remove it or supply a handler. Any explicitly
//! supplied non-null on* callback, including a dynamic callback, is retained.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryGestureDetector;

impl Rule for AvoidUnnecessaryGestureDetector {
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
        let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "GestureDetector") {
            return;
        }
        let matches = {
            !args.named().any(|arg| {
                arg.name().name().starts_with("on")
                    && !matches!(arg.value().unparenthesized(), Expr::NullLit(_))
            })
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-unnecessary-gesture-detector",
                "Remove this GestureDetector or provide an event handler.",
                crate::lint::flutter::constructor_span(node),
            ));
        }
    }
}
