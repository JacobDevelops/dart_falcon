//! Prefer direct invocation of functions and resolved callable instances.
//!
//! Null-aware calls remain necessary for nullable receivers. Dynamic, unresolved,
//! static and extension-only call members are deliberately not diagnosed.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, MemberKind, ResolvedTypeView, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryCall;

impl Rule for AvoidUnnecessaryCall {
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
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else { return };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return;
        };
        if field.field().name() != "call" || field.is_null_safe() || state.inside_cascade {
            return;
        }
        let receiver = state.infer(field.object());
        let callable_instance = matches!(receiver.view(), ResolvedTypeView::Interface { .. })
            && !receiver.nullable()
            && state
                .signatures
                .resolved_member_facts(&receiver, "call", &state.model)
                .is_some_and(|facts| {
                    facts
                        .iter()
                        .any(|fact| fact.kind == MemberKind::Method && !fact.is_static)
                });
        if matches!(
            receiver.view(),
            ResolvedTypeView::Function {
                nullable: false,
                ..
            }
        ) || receiver.interface("dart:core", "Function") && !receiver.nullable()
            || callable_instance
        {
            self.diagnostics.push(self.ctx.diagnostic("Invoke this function directly instead of using `.call()`.", field.field().span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
