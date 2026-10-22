//! Riverpod ownership classification and the shared mounted/async-gap analysis.

use falcon_analyze::{AnalyzeContext, ResolvedType, RulePreparation};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::riverpod::is_exact_type;
use crate::lint::support::async_semantics::{
    Semantics as AsyncFacts, constant_bool, equality_operands, mounted_kind, mounted_receiver_kind,
};
use crate::lint::support::async_state::async_gap_spans;

#[derive(Clone, Copy)]
pub(crate) enum Target {
    WidgetRead,
    Notifier,
}

pub(crate) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    target: Target,
) -> RulePreparation<'a> {
    RulePreparation::Semantic(Box::new(Collector {
        program,
        ctx,
        target,
        facts: AsyncFacts::default(),
        mounted_receivers: Vec::new(),
    }))
}

struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    target: Target,
    facts: AsyncFacts,
    mounted_receivers: Vec<NodeId>,
}

fn member_owner(ty: &ResolvedType, name: &str, state: &SemanticState<'_>) -> Option<ResolvedType> {
    state
        .signatures
        .resolved_member_owner(ty, name, &state.model)
        .map(ResolvedType::interface_identity)
}

fn own_notifier_member(
    ctx: &AnalyzeContext<'_>,
    expression: Expr<'_>,
    state: &SemanticState<'_>,
    names: &[&str],
) -> bool {
    let name = match expression.unparenthesized() {
        Expr::Ident(identifier) if !state.environment.is_bound(identifier.name()) => {
            identifier.name()
        }
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
            field.field().name()
        }
        _ => return false,
    };
    if !names.contains(&name) {
        return false;
    }
    let Some(enclosing) = &state.enclosing_type else {
        return false;
    };
    let owner = member_owner(enclosing, name, state).unwrap_or_else(|| enclosing.clone());
    is_exact_type(ctx, &owner, &["AnyNotifier"])
}

fn widget_ref<'a>(
    ctx: &AnalyzeContext<'_>,
    expression: Expr<'a>,
    state: &SemanticState<'a>,
) -> bool {
    is_exact_type(ctx, &state.infer(expression), &["WidgetRef"])
}

fn widget_mounted<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    match expression.unparenthesized() {
        Expr::Ident(identifier)
            if identifier.name() == "mounted" && !state.environment.is_bound("mounted") =>
        {
            state
                .enclosing_type
                .as_ref()
                .and_then(|ty| mounted_kind(ty, state))
                .is_some()
        }
        Expr::Field(field) if field.field().name() == "mounted" => {
            mounted_receiver_kind(field.object(), state).is_some()
        }
        _ => false,
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let id = expression.syntax().id();
        if matches!(expression, Expr::Ident(_) | Expr::Field(_) | Expr::Call(_))
            && state.infer(expression) == ResolvedType::Never
        {
            self.facts.never.insert(id);
        }
        for operand in equality_operands(expression).into_iter().flatten() {
            let operand = operand.unparenthesized();
            if let Some(value) = constant_bool(operand, state, self.ctx.constant_cache()) {
                self.facts.constants.insert(operand.syntax().id(), value);
            }
        }
        match self.target {
            Target::WidgetRead => {
                if widget_mounted(expression, state) {
                    self.facts.mounted.insert(id, true);
                }
                if let Expr::Call(call) = expression
                    && let Expr::Field(field) = call.callee().unparenthesized()
                    && field.field().name() == "read"
                    && widget_ref(self.ctx, field.object(), state)
                {
                    self.facts.uses.insert(id, true);
                }
            }
            Target::Notifier => {
                if let Expr::Field(field) = expression
                    && field.field().name() == "mounted"
                    && own_notifier_member(self.ctx, field.object(), state, &["ref"])
                {
                    self.facts.mounted.insert(id, true);
                    self.mounted_receivers
                        .push(field.object().unparenthesized().syntax().id());
                } else if matches!(expression, Expr::Ident(_) | Expr::Field(_))
                    && own_notifier_member(self.ctx, expression, state, &["ref", "state"])
                {
                    self.facts.uses.insert(id, true);
                }
            }
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        for receiver in &self.mounted_receivers {
            self.facts.uses.remove(receiver);
        }
        let message = match self.target {
            Target::WidgetRead => {
                "Check that the widget is mounted before calling `ref.read` after an asynchronous gap."
            }
            Target::Notifier => {
                "Check `ref.mounted` before using notifier `ref` or `state` after an async gap."
            }
        };
        async_gap_spans(self.program, self.facts)
            .into_iter()
            .map(|span| self.ctx.diagnostic(message, span))
            .collect()
    }
}
