//! Flags Bloc `add` and `emit` calls after an await without a fresh `isClosed` check.
//!
//! A Bloc or Cubit can close while asynchronous work is suspended. Check
//! `!isClosed` before dispatching or emitting, or return when `isClosed` is true.
//! Each await invalidates earlier checks. Both guarded branches and terminating
//! early-return guards are recognized, and nested functions are independent.
//! Awaits in call arguments invalidate even an enclosing guard.
//! Calls must target the enclosing package-backed Bloc/Cubit, or an Emitter
//! parameter of its event handler. An Emitter also accepts its own `!isDone`
//! guard. Unrelated receivers, shadowed names and opaque mixins are ignored.
//! Analysis follows straight-line statements and enclosing if branches;
//! uncertain loops, switch/try flow, pattern conditions, conditional collection
//! elements and optional/short-circuit evaluation are not treated as proof of
//! an unguarded asynchronous path.
//!
//! ### Bad
//! ```dart
//! await load();
//! emit(Loaded());
//! ```
//!
//! ### Good
//! ```dart
//! await load();
//! if (isClosed) return;
//! emit(Loaded());
//! ```

use std::collections::{HashMap, HashSet};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::bloc::{BlocTypes, own_member};
use crate::lint::support::async_state::{AsyncGapFacts, async_gap_spans_with_guard};

#[derive(Default)]
pub struct CheckIsNotClosedAfterAsyncGap;

impl Rule for CheckIsNotClosedAfterAsyncGap {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            program,
            types: BlocTypes::new(program),
            calls: HashMap::new(),
            closed: HashSet::new(),
            done: HashMap::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    types: BlocTypes<'a>,
    calls: HashMap<NodeId, Option<Span>>,
    closed: HashSet<NodeId>,
    done: HashMap<NodeId, Span>,
}

impl<'a> Collector<'a> {
    fn emitter(&self, callee: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let Expr::Ident(identifier) = callee.unparenthesized() else {
            return false;
        };
        let name = identifier.name();
        if !state.environment.is_bound(name) {
            return false;
        }
        let Some(class) = callee.syntax().ancestors().find_map(ClassDecl::cast) else {
            return false;
        };
        if !self
            .types
            .is_bloc_class(class, &state.model, Some(&state.signatures))
        {
            return false;
        }
        if self
            .types
            .is_api_type(&state.environment.lookup(name), &state.model, "Emitter")
        {
            return true;
        }
        let declaration = state.environment.binding_declaration(name);
        if self
            .types
            .binding_has_api_syntax(callee, state, &["Emitter"])
        {
            return true;
        }
        // `on<Event>((event, emit) ...)` contextually supplies an Emitter even
        // when the external package's declarations are unavailable.
        let Some(function) =
            callee
                .syntax()
                .ancestors()
                .find_map(|ancestor| match Expr::cast(ancestor) {
                    Some(Expr::FuncExpr(function)) => Some(function),
                    _ => None,
                })
        else {
            return false;
        };
        let Some(parameter) = function.params().params().nth(1) else {
            return false;
        };
        Some(parameter.name().span()) == declaration
            && function
                .syntax()
                .ancestors()
                .skip(1)
                .find_map(|ancestor| match Expr::cast(ancestor) {
                    Some(Expr::Call(call)) => Some(call),
                    _ => None,
                })
                .is_some_and(|call| own_member(call.callee(), "on", &self.types, state))
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let Self {
            ctx,
            program,
            calls,
            closed,
            done,
            ..
        } = *self;
        async_gap_spans_with_guard(
            program,
            AsyncGapFacts::default(),
            calls,
            true,
            |expression, emitter| {
                (closed.contains(&expression)
                    || emitter.as_ref().is_some_and(|binding| done.get(&expression) == Some(binding)))
                .then_some(false)
            },
        )
        .into_iter()
        .map(|span| ctx.diagnostic(
            "Check `isClosed` after the `await` before adding an event or emitting a state.",
            span,
        ))
        .collect()
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if own_member(node, "isClosed", &self.types, state) {
            self.closed.insert(node.syntax().id());
        }
        if let Expr::Field(field) = node
            && field.field().name() == "isDone"
            && self.emitter(field.object(), state)
            && let Expr::Ident(identifier) = field.object().unparenthesized()
            && let Some(binding) = state.environment.binding_declaration(identifier.name())
        {
            self.done.insert(node.syntax().id(), binding);
        }
        if let Expr::Call(call) = node {
            let emitter = self.emitter(call.callee(), state);
            if !emitter
                && !own_member(call.callee(), "add", &self.types, state)
                && !own_member(call.callee(), "emit", &self.types, state)
            {
                return;
            }
            let binding = if emitter
                && let Expr::Ident(identifier) = call.callee().unparenthesized()
            {
                state.environment.binding_declaration(identifier.name())
            } else {
                None
            };
            self.calls.insert(call.syntax().id(), binding);
        }
    }
}

