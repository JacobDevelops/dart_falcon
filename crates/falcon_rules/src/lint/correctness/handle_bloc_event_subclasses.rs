//! Warns when a Bloc constructor leaves a concrete event subclass without a handler.
//!
//! Register a handler for every event subclass, or register a handler for their
//! parent event. This rule checks event classes declared in the same file and
//! `on<Event>` registrations in each generative constructor, including cascades.
//! Handler coverage compares full event instantiations and accepts unknown subtyping.
//! Inferred registrations conservatively count as coverage. Factories, redirects,
//! abstract event classes, generic event roots and overridden `on` APIs are ignored.
//! Calls to instance helpers on `this`, including inherited helpers, count as
//! unresolved coverage.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::bloc::BlocTypes;
use crate::lint::bloc::own_member;

#[derive(Default)]
pub struct HandleBlocEventSubclasses;

impl Rule for HandleBlocEventSubclasses {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            program,
            types: BlocTypes::new(program),
            constructors: Vec::new(),
        }))
    }
}

struct ConstructorEvents {
    id: NodeId,
    span: Span,
    missing: Vec<(DeclarationIdentity, ResolvedType)>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    types: BlocTypes<'a>,
    constructors: Vec<ConstructorEvents>,
}

impl<'a> Collector<'a> {
    fn register(
        &mut self,
        syntax: SyntaxNode<'a>,
        event: Option<DartType<'a>>,
        state: &SemanticState<'a>,
    ) {
        let Some(constructor) = syntax.ancestors().find_map(ConstructorDecl::cast) else {
            return;
        };
        let Some(events) = self
            .constructors
            .iter_mut()
            .find(|entry| entry.id == constructor.syntax().id())
        else {
            return;
        };
        let Some(event) = event else {
            // Without an explicit type argument, the inferred handler may cover
            // the root event. Prefer missing a warning to rejecting that handler.
            events.missing.clear();
            return;
        };
        let handled = state.model.resolve_type_in(event, &state.type_parameters);
        if !matches!(handled, ResolvedType::Interface { .. }) {
            return;
        }
        events
            .missing
            .retain(|(_, ty)| state.signatures.relation(ty, TypeRelation::SubtypeOf(&handled), &state.model) == Truth::No);
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_constructor(&mut self, constructor: ConstructorDecl<'a>, state: &SemanticState<'a>) {
        if constructor.is_factory()
            || constructor.redirect().is_some()
            || constructor
                .initializers()
                .any(|initializer| matches!(initializer, ConstructorInitializer::ThisCall(_)))
        {
            return;
        }
        let Some(class) = constructor.syntax().ancestors().find_map(ClassDecl::cast) else {
            return;
        };
        let Some(supertype) = class.extends() else {
            return;
        };
        if !self.types.is_api_syntax(supertype, &state.model, "Bloc")
            || self.types.has_local_member(class, &state.model, "on", true)
        {
            return;
        }
        let DartType::Named(named) = supertype else {
            return;
        };
        let Some(event) = named.type_args().next() else {
            return;
        };
        let ResolvedType::Interface {
            identity: root,
            arguments,
            ..
        } = state.model.resolve_type_in(event, &state.type_parameters)
        else {
            return;
        };
        if !arguments.is_empty() {
            return;
        }
        let mut missing = Vec::with_capacity(self.program.declarations().size_hint().0);
        for declaration in self.program.declarations() {
            let TopLevelDecl::Class(event_class) = declaration else {
                continue;
            };
            if event_class.modifiers().is_abstract
                || matches!(
                    event_class.modifiers().restriction,
                    Some(ClassRestriction::Sealed)
                )
            {
                continue;
            }
            let Some(identity) = state.model.resolve_name(event_class.name().name()) else {
                continue;
            };
            let mut ty = ResolvedType::interface_identity(identity.clone());
            if let Some(count) = state.signatures.type_parameter_count(&identity)
                && count > 0
                && let ResolvedType::Interface { arguments, .. } = &mut ty
            {
                // A generic declaration represents unknown instantiations, not dynamic.
                *arguments = vec![ResolvedType::Unknown; count].into();
            }
            if identity != root
                && state.signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&root), &state.model) == Truth::Yes
            {
                missing.push((identity, ty));
            }
        }
        if !missing.is_empty() {
            self.constructors.push(ConstructorEvents {
                id: constructor.syntax().id(),
                span: constructor.span(),
                missing,
            });
        }
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expression else {
            return;
        };
        if !own_member(call.callee(), "on", &self.types, state) {
            let helper = match call.callee().unparenthesized() {
                Expr::Ident(identifier) => state
                    .implicit_invocation_receiver(identifier.name())
                    .is_some(),
                Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
                    state
                        .signatures
                        .resolved_member(
                            &state.infer(field.object()),
                            field.field().name(),
                            &state.model,
                        )
                        .is_some()
                }
                _ => false,
            };
            if helper {
                self.register(call.syntax(), None, state);
            }
            return;
        }
        let mut arguments = call.type_args();
        let event = arguments.next();
        if arguments.next().is_some() {
            return;
        }
        self.register(call.syntax(), event, state);
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        let CascadeOp::Call(call) = operation else {
            return;
        };
        if call.name().name() != "on" {
            return;
        }
        let Some(section) = call.syntax().ancestors().find_map(CascadeSection::cast) else {
            return;
        };
        if section
            .ops()
            .next()
            .is_none_or(|first| first.syntax() != call.syntax())
            || section
                .syntax()
                .ancestors()
                .find_map(CascadeExpr::cast)
                .is_none_or(|cascade| !matches!(cascade.object().unparenthesized(), Expr::This(_)))
        {
            return;
        }
        // Constructor collection already excludes local overrides and opaque
        // mixins. Apply the same resolved package-owner guard as own_member.
        if let Some(owner) =
            state
                .signatures
                .resolved_readable_member_owner(receiver, "on", &state.model)
            && !owner.is_package("bloc", owner.name())
            && !owner.is_package("flutter_bloc", owner.name())
            && !state.model.declaration_uri(&owner).is_some_and(|uri| {
                uri.starts_with("package:bloc/") || uri.starts_with("package:flutter_bloc/")
            })
        {
            return;
        }
        let mut arguments = call.type_args();
        let event = arguments.next();
        if arguments.next().is_some() {
            return;
        }
        self.register(call.syntax(), event, state);
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.constructors.into_iter().filter(|entry| !entry.missing.is_empty()).map(|entry| {
            self.ctx.diagnostic("This Bloc leaves event subclasses unhandled. Register their handlers or handle the parent event.", entry.span)
        }).collect()
    }
}
