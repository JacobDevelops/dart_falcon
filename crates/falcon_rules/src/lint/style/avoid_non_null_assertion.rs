//! Flags the null-assertion operator `!` except on a resolved `Map` index.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidNonNullAssertion;

impl Rule for AvoidNonNullAssertion {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
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

impl<'a> SemanticRuleVisitor<'a> for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: cst::Expr<'_>, state: &SemanticState<'_>) {
        let cst::Expr::NullAssert(assertion) = node else {
            return;
        };
        let operand = assertion.operand();
        if let cst::Expr::Index(index) = operand.unparenthesized() {
            let receiver = state.infer(index.object());
            if state
                .signatures
                .instantiated_supertype(&receiver, "dart:core", "Map", &state.model)
                .is_some()
            {
                return;
            }
            if !known_non_map(&receiver) {
                return;
            }
        }
        self.diags.push(self.ctx.diagnostic("Avoid using the null assertion operator '!'", assertion.span()));
    }
}

fn known_non_map(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Null
            | ResolvedType::Void
            | ResolvedType::Never
            | ResolvedType::Function { .. }
            | ResolvedType::Record { .. }
            | ResolvedType::Interface {
                identity: falcon_analyze::DeclarationIdentity::Sdk { .. },
                ..
            }
    )
}
