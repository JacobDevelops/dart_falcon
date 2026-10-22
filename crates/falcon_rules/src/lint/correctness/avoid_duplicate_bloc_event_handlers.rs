//! Flags later registrations of the same event type in a Bloc constructor.
//!
//! Bloc permits only one `on<Event>` handler per event type. Combine the handlers
//! instead of registering the same event twice. This rule checks unconditional
//! constructor registrations on `this`, including unqualified calls, separately
//! for each constructor. Deferred callbacks, branches, other receivers, local
//! `on` bindings, unresolved generic arguments, opaque mixins and classes
//! overriding `on` are excluded. Bloc identity requires package resolution or
//! a matching bloc/flutter_bloc import and superclass.
//!
//! ### Bad
//! ```dart
//! on<Started>((event, emit) {});
//! on<Started>((event, emit) {}); // A second handler for Started.
//! ```
//!
//! ### Good
//! ```dart
//! on<Started>((event, emit) { initialize(); refresh(); });
//! ```

use std::collections::HashMap;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::bloc::{BlocTypes, own_member};

#[derive(Default)]
pub struct AvoidDuplicateBlocEventHandlers;

impl Rule for AvoidDuplicateBlocEventHandlers {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            types: BlocTypes::new(program),
            registrations: HashMap::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    types: BlocTypes<'a>,
    registrations: HashMap<NodeId, Vec<ResolvedType>>,
    diagnostics: Vec<Diagnostic>,
}


impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        if !own_member(call.callee(), "on", &self.types, state) {
            return;
        }
        let mut arguments = call.type_args();
        let Some(event) = arguments.next() else {
            return;
        };
        if arguments.next().is_some() {
            return;
        }
        let Some(constructor) = call.syntax().ancestors().find_map(ConstructorDecl::cast) else {
            return;
        };
        // Only direct constructor statements prove that both registrations run.
        let Some(statement) = call.syntax().parent().and_then(Stmt::cast) else {
            return;
        };
        if !matches!(statement, Stmt::Expr(_))
            || statement
                .syntax()
                .parent()
                .and_then(Block::cast)
                .is_none_or(|block| block.syntax().parent() != Some(constructor.syntax()))
        {
            return;
        }
        let event_type = state.model.resolve_type_in(event, &state.type_parameters);
        if !fully_known(&event_type) {
            return;
        }
        let registered = self
            .registrations
            .entry(constructor.syntax().id())
            .or_default();
        if registered.contains(&event_type) {
            self.diagnostics.push(self.ctx.diagnostic("This event already has a handler. Combine the handlers into one registration.", event.span()));
        } else {
            registered.push(event_type);
        }
    }
}

fn fully_known(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Unknown | ResolvedType::Dynamic => false,
        ResolvedType::Interface { arguments, .. } => arguments.iter().all(fully_known),
        ResolvedType::Record {
            positional, named, ..
        } => positional.iter().all(fully_known) && named.iter().all(|(_, ty)| fully_known(ty)),
        ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } => {
            fully_known(return_type)
                && positional.iter().all(fully_known)
                && named.iter().all(|(_, ty)| fully_known(ty))
        }
        _ => true,
    }
}
