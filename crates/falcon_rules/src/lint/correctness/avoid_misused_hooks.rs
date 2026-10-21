//! Flags confirmed Flutter hooks called directly in non-hook Flutter widget builds.
//!
//! Widget ancestry must resolve. HookWidget/HookConsumerWidget builds and State
//! builds for StatefulHookWidget/StatefulHookConsumerWidget are excluded, as is
//! unknown ancestry. Custom useX functions must compose resolved flutter_hooks
//! calls; spelling alone is insufficient. Nested callbacks belong to
//! avoid-hooks-outside-build instead.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_hooks_outside_build::HookContexts;
use crate::lint::flutter_hooks::HookCalls;

#[derive(Default)]
pub struct AvoidMisusedHooks;

impl Rule for AvoidMisusedHooks {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            contexts: HookContexts::new(program),
            calls: HookCalls::new(program),
            candidates: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    contexts: HookContexts<'a>,
    calls: HookCalls<'a>,
    candidates: Vec<Expr<'a>>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        self.calls.observe(expr, state);
        if matches!(expr, Expr::Call(_)) && self.contexts.non_hook_build(expr, state) {
            self.candidates.push(expr);
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.calls.finish();
        self.candidates.iter().filter(|&&expr| self.calls.contains(expr)).map(|expr| {
            self.ctx.diagnostic("Hooks require a HookWidget, HookConsumerWidget, or HookBuilder; replace this non-hook widget.", expr.span())
        }).collect()
    }
}
