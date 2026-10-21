//! Prefer the cryptographically secure `dart:math` random generator.
//!
//! Only resolved unnamed Random constructors are checked; explicit seeds are
//! exempt because they express a requirement for reproducible randomness.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferRandomSecure;

impl Rule for PreferRandomSecure {
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
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let resolved = match node {
            Expr::Call(call)
                if call.args().positional().next().is_none()
                    && call.args().named().next().is_none() =>
            {
                let callee = call.callee().unparenthesized();
                let Some(segments) = falcon_analyze::expression_segments(callee, true) else {
                    return;
                };
                if state.environment.is_bound(segments[0]) {
                    return;
                }
                falcon_analyze::resolve_call_constructor(callee, &state.model, true)
            }
            Expr::New(new)
                if new.args().positional().next().is_none()
                    && new.args().named().next().is_none() =>
            {
                falcon_analyze::resolve_new_constructor(
                    new.dart_type(),
                    new.constructor_name(),
                    &state.model,
                )
            }
            _ => return,
        };
        if let Some((identity, "new")) = resolved
            && state
                .model
                .unaliased(identity)
                .is_sdk("dart:math", "Random")
        {
            self.diags.push(self.ctx.diagnostic("Use Random.secure() when reproducible randomness is not required.", node.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
