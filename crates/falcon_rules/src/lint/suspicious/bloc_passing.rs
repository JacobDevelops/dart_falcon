//! Resolved argument checks shared by the Bloc dependency rules.

use crate::lint::bloc::BlocTypes;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Clone, Copy)]
pub(super) enum Passing {
    BlocToBloc,
    ContextToBloc,
    BlocToWidget,
}

pub(super) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    passing: Passing,
) -> RulePreparation<'a> {
    let types = BlocTypes::new(program);
    let events = None;
    RulePreparation::Semantic(Box::new(Collector {
        ctx,
        types,
        events,
        passing,
        diagnostics: Vec::new(),
    }))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    types: BlocTypes<'a>,
    passing: Passing,
    events: Option<Vec<DeclarationIdentity>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let (target, args) = if let Some((identity, constructor, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        {
            if !state.signatures.has_constructor(&identity, constructor) {
                return;
            }
            if matches!(self.passing, Passing::BlocToWidget)
                && self.ctx.identities().is_some_and(|index| {
                    [
                        "BlocProvider",
                        "RepositoryProvider",
                        "BlocBuilder",
                        "BlocListener",
                        "BlocConsumer",
                        "BlocSelector",
                    ]
                    .into_iter()
                    .any(|name| index.is_package_declaration(&identity, "flutter_bloc", name))
                        || index.is_package_declaration(&identity, "provider", "Provider")
                })
            {
                return;
            }
            (ResolvedType::interface_identity(identity), args)
        } else if !matches!(self.passing, Passing::BlocToWidget)
            && let Expr::Call(call) = node
        {
            let receiver = match call.callee().unparenthesized() {
                Expr::Field(field) => state.infer(field.object()),
                Expr::Ident(identifier) => {
                    let Some(receiver) = state.implicit_invocation_receiver(identifier.name())
                    else {
                        return;
                    };
                    receiver
                }
                _ => return,
            };
            (receiver, call.args())
        } else {
            return;
        };
        self.check_arguments(&target, args, state);
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if !matches!(self.passing, Passing::BlocToWidget)
            && let CascadeOp::Call(call) = operation
        {
            self.check_arguments(receiver, call.args(), state);
        }
    }
}

impl<'a> Collector<'a> {
    fn check_arguments(
        &mut self,
        target: &ResolvedType,
        args: ArgList<'a>,
        state: &SemanticState<'a>,
    ) {
        if !args
            .positional()
            .chain(args.named().map(|argument| argument.value()))
            .any(|argument| self.matches_argument(argument, state))
            || !self.matches_target(target, state)
        {
            return;
        }
        for argument in args
            .positional()
            .chain(args.named().map(|argument| argument.value()))
        {
            if self.matches_argument(argument, state) {
                let message = match self.passing {
                    Passing::BlocToBloc => {
                        "Avoid coupling blocs by passing a Bloc or Cubit to another bloc."
                    }
                    Passing::ContextToBloc => {
                        "Keep Flutter BuildContext out of Bloc and Cubit dependencies."
                    }
                    Passing::BlocToWidget => {
                        "Obtain the Bloc or Cubit from the widget context instead of passing it directly."
                    }
                };
                self.diagnostics
                    .push(self.ctx.diagnostic(message, argument.span()));
            }
        }
    }

    fn matches_argument(&self, argument: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let ty = state.infer(argument);
        match self.passing {
            Passing::ContextToBloc => {
                state.signatures.is_subtype_of(
                    &ty,
                    &DeclarationIdentity::flutter("BuildContext"),
                    &state.model,
                ) == Truth::Yes
            }
            Passing::BlocToBloc | Passing::BlocToWidget => {
                self.types
                    .is_bloc_type(&ty, &state.model, Some(&state.signatures))
            }
        }
    }

    fn matches_target(&mut self, target: &ResolvedType, state: &SemanticState<'a>) -> bool {
        match self.passing {
            Passing::BlocToWidget => {
                state.signatures.is_subtype_of(
                    target,
                    &DeclarationIdentity::flutter("Widget"),
                    &state.model,
                ) == Truth::Yes
            }
            Passing::BlocToBloc => {
                self.types
                    .is_bloc_type(target, &state.model, Some(&state.signatures))
            }
            Passing::ContextToBloc => {
                self.types
                    .is_bloc_type(target, &state.model, Some(&state.signatures))
                    || self
                        .events
                        .get_or_insert_with(|| Self::event_types(state))
                        .iter()
                        .any(|event| {
                            state.signatures.is_subtype_of(target, event, &state.model)
                                == Truth::Yes
                        })
            }
        }
    }

    fn event_types(state: &SemanticState<'a>) -> Vec<DeclarationIdentity> {
        state
            .signatures
            .declaration_identities()
            .filter(|identity| state.model.identities().is_project(identity))
            .filter_map(|identity| {
                let bloc = state.signatures.instantiated_supertype_matching(
                    &ResolvedType::interface_identity(identity.clone()),
                    |identity| crate::lint::bloc::is_api_identity(identity, &state.model, "Bloc"),
                    &state.model,
                )?;
                match bloc.arguments().first()? {
                    ResolvedType::Interface { identity, .. }
                        if state.model.identities().is_project(identity) =>
                    {
                        Some(identity.clone())
                    }
                    _ => None,
                }
            })
            .fold(Vec::new(), |mut events, event| {
                if !events.contains(&event) {
                    events.push(event);
                }
                events
            })
    }
}
