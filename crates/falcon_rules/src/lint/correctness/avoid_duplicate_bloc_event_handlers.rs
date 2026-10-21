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

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::bloc::BlocTypes;

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

pub(super) fn own_member<'a>(
    expression: Expr<'a>,
    name: &str,
    types: &BlocTypes<'a>,
    state: &SemanticState<'a>,
) -> bool {
    let matches = match expression.unparenthesized() {
        Expr::Ident(identifier) => {
            identifier.name() == name
                && !state.environment.is_bound(name)
                && !matches!(
                    state.model.resolve_library_name(name),
                    falcon_analyze::DocName::Declaration { .. }
                        | falcon_analyze::DocName::Extension
                )
        }
        Expr::Field(field) => {
            field.field().name() == name
                && matches!(
                    field.object().unparenthesized(),
                    Expr::This(_) | Expr::Super(_)
                )
        }
        _ => false,
    };
    if !matches {
        return false;
    }
    let Some(class) = expression.syntax().ancestors().find_map(ClassDecl::cast) else {
        return false;
    };
    if expression
        .syntax()
        .ancestors()
        .filter_map(MethodDecl::cast)
        .any(|method| method.is_static())
    {
        return false;
    }
    let is_super = matches!(expression.unparenthesized(),
        Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::Super(_)));
    let receiver = state
        .model
        .resolve_name(class.name().name())
        .map(ResolvedType::interface_identity)
        .unwrap_or(ResolvedType::Unknown);
    let owner = if is_super {
        state
            .signatures
            .resolved_super_member_owner(&receiver, name, false, &state.model)
    } else {
        state
            .signatures
            .resolved_readable_member_owner(&receiver, name, &state.model)
    };
    if let Some(owner) = owner
        && !owner.is_package("bloc", owner.name())
        && !owner.is_package("flutter_bloc", owner.name())
        && !state.model.declaration_uri(&owner).is_some_and(|uri| {
            uri.starts_with("package:bloc/") || uri.starts_with("package:flutter_bloc/")
        })
    {
        return false;
    }
    types.is_bloc_class(class, &state.model, Some(&state.signatures))
        && !types.has_local_member(class, &state.model, name, !is_super)
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
