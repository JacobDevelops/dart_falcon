//! Flags Bloc and Cubit emissions that reuse their current state instance.
//!
//! Re-emitting the same object, including a cascade that mutates that object,
//! can suppress state updates. Create a new state (for example with `copyWith`)
//! instead. Checks require a confirmed `bloc` or `flutter_bloc` API owner and
//! recognize inherited `emit`, `this.emit`, `super.emit`, and Bloc `on` handler
//! emitter parameters, including `Emitter`-typed parameters in tear-off handlers.
//! Local bindings and overridden `emit`/`state` members are
//! excluded. Only direct `state`, `this.state`, `super.state`, parentheses and
//! cascades rooted in those reads are checked; arbitrary aliases are not traced.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_duplicate_bloc_event_handlers::own_member;
use crate::lint::bloc::BlocTypes;

#[derive(Default)]
pub struct EmitNewBlocStateInstances;

impl Rule for EmitNewBlocStateInstances {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            types: BlocTypes::new(program),
            emitters: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    types: BlocTypes<'a>,
    emitters: Vec<Span>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn current_state(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let mut expression = expression.unparenthesized();
        while let Expr::Cascade(cascade) = expression {
            expression = cascade.object().unparenthesized();
        }
        own_member(expression, "state", &self.types, state)
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expression else {
            return;
        };
        let callee = call.callee().unparenthesized();
        let candidate = match callee {
            Expr::Ident(name) => {
                matches!(name.name(), "emit" | "on")
                    || state.environment.binding_declaration(name.name()).is_some()
            }
            Expr::Field(field) => matches!(field.field().name(), "emit" | "on"),
            _ => false,
        };
        if !candidate {
            return;
        }
        let Some(class) = callee.syntax().ancestors().find_map(ClassDecl::cast) else {
            return;
        };
        if !self
            .types
            .is_bloc_class(class, &state.model, Some(&state.signatures))
        {
            return;
        }
        let is_on = own_member(callee, "on", &self.types, state)
            && self
                .types
                .is_event_bloc_class(class, &state.model, Some(&state.signatures));
        let is_emit = own_member(callee, "emit", &self.types, state);
        if is_on
            && let Some(Expr::FuncExpr(handler)) =
                call.args().positional().next().map(Expr::unparenthesized)
            && let Some(emitter) = handler.params().positional().nth(1)
        {
            self.emitters.push(emitter.name().span());
        }
        let is_emitter = match callee {
            Expr::Ident(name) => state
                .environment
                .binding_declaration(name.name())
                .is_some_and(|declaration| {
                    self.emitters.contains(&declaration)
                        || self
                            .types
                            .is_api_type(&state.infer(callee), &state.model, "Emitter")
                        || self
                            .types
                            .binding_has_api_syntax(callee, state, &["Emitter"])
                }),
            _ => false,
        };
        if !is_emitter && !is_emit {
            return;
        }
        let mut arguments = call.args().positional();
        if let Some(argument) = arguments.next()
            && arguments.next().is_none()
            && call.args().named().is_empty()
            && self.current_state(argument, state)
        {
            self.diagnostics.push(self.ctx.diagnostic("Create a new state instance instead of emitting the current state.", argument.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
