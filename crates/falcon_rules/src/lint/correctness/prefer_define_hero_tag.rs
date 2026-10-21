//! Require an explicit heroTag on Flutter FloatingActionButton constructors.
//!
//! The default tag is shared, so multiple buttons can collide during a Hero
//! transition. Set a unique tag or explicitly pass null to disable the transition.
//! Includes unnamed, small, large, and extended constructors; package identities
//! exclude local lookalikes and shadowed names. Matches the DCM rule of this id.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferDefineHeroTag;

impl Rule for PreferDefineHeroTag {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
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
        let Some((identity, _, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "FloatingActionButton") {
            return;
        }
        if !args
            .named()
            .any(|argument| argument.name().name() == "heroTag")
        {
            self.diags.push(self.ctx.diagnostic("Provide an explicit heroTag for this floating action button.", crate::lint::flutter::constructor_span(node)));
        }
    }
}
