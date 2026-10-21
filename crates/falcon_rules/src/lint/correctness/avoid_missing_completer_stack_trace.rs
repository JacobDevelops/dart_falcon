//! Preserve error origins when calling SDK `Completer.completeError`.
//! Pass the caught stack trace, or `StackTrace.current`, as the second argument.
//!
//! Checks ordinary, null-aware, and cascade calls using resolved receiver types.
//! Explicit null is missing too. Custom same-named methods and dynamic receivers
//! are ignored; inherited methods are checked only when their SDK owner resolves
//! and library/import lookup does not select a different completeError function.
//!
//! Reference: https://dcm.dev/docs/rules/common/avoid-missing-completer-stack-trace/

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMissingCompleterStackTrace;

impl Rule for AvoidMissingCompleterStackTrace {
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

impl<'a> Collector<'a> {
    fn check(
        &mut self,
        receiver: &ResolvedType,
        args: ArgList<'a>,
        span: Span,
        state: &SemanticState<'_>,
    ) {
        let sdk_method = receiver.interface("dart:async", "Completer")
            || state
                .signatures
                .resolved_member_owner(receiver, "completeError", &state.model)
                .is_some_and(|owner| owner.is_sdk("dart:async", "Completer"));
        let positional = args.positional();
        if sdk_method
            && args.named().is_empty()
            && !positional.is_empty()
            && (positional.len() == 1
                || args
                    .positional()
                    .nth(1)
                    .is_some_and(|argument| matches!(argument.unparenthesized(), Expr::NullLit(_))))
        {
            self.diagnostics.push(self.ctx.diagnostic("Pass a stack trace as the second argument to Completer.completeError.", span));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        let Expr::Call(call) = node else {
            return;
        };
        match call.callee().unparenthesized() {
            Expr::Field(field) if field.field().name() == "completeError" => {
                self.check(
                    &state.infer(field.object()),
                    call.args(),
                    call.span(),
                    state,
                );
            }
            Expr::Ident(name)
                if name.name() == "completeError"
                    && !state.environment.is_bound(name.name())
                    && !state.constant_owner.as_ref().is_some_and(|owner| {
                        !state
                            .signatures
                            .declared_member_facts(owner, name.name())
                            .is_empty()
                    })
                    && state.model.resolve_value(name.name()).is_none() =>
            {
                if let Some(receiver) = state.enclosing_type.as_ref() {
                    self.check(receiver, call.args(), call.span(), state);
                }
            }
            _ => {}
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'_>,
    ) {
        if let CascadeOp::Call(call) = operation
            && call.name().name() == "completeError"
        {
            self.check(receiver, call.args(), call.span(), state);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
