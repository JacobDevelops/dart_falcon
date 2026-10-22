//! Requires expectLater when a canonical expect call receives an unawaited Future.

use crate::lint::support::test_assertions;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferExpectLater;

impl Rule for PreferExpectLater {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !test_assertions::enabled(program, ctx) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Some(("expect", package)) = test_assertions::package_call(call, state) else {
            return;
        };
        if !test_assertions::MATCHER_PACKAGES.contains(&package) {
            return;
        }
        let Some(actual) = call.args().positional().next() else {
            return;
        };
        if state
            .signatures
            .implements_sdk(&state.infer(actual), "dart:async", "Future")
            == falcon_analyze::Truth::Yes
        {
            self.diags.push(self.ctx.diagnostic("Use `expectLater` and `await` its result when expecting a `Future`.", actual.span()));
        }
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
