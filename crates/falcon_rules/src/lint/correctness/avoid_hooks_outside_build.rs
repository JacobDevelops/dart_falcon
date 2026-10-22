//! Flags confirmed Flutter hooks outside hook builds, custom hooks, and HookBuilder/HookConsumer builders.
//!
//! Only resolved flutter_hooks functions and useX/_useX functions that actually
//! compose them count as hooks. Nested callbacks do not inherit the enclosing
//! build or custom-hook permission. Confirmed non-hook widget builds are left
//! to avoid-misused-hooks; unresolved calls, unresolved build ancestry, and shadowed names are ignored.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter_hooks::HookCalls;
use crate::lint::support::hook_contexts::HookContexts;

#[derive(Default)]
pub struct AvoidHooksOutsideBuild;

impl Rule for AvoidHooksOutsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            contexts: HookContexts::new(program, ctx),
            calls: HookCalls::new(program, ctx),
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
        self.contexts.observe(expr, state);
        if matches!(expr, Expr::Call(_)) && !self.contexts.allowed(expr, state) {
            self.candidates.push(expr);
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.calls.finish();
        self.candidates.iter().filter(|&&expr| self.calls.contains(expr)).map(|expr| {
            self.ctx.diagnostic("Call hooks directly in a hook widget `build`, custom hook, or `HookBuilder`/`HookConsumer` builder.", expr.span())
        }).collect()
    }
}
