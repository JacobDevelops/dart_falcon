//! Resolved argument checks shared by the Bloc dependency rules.

use crate::lint::bloc::BlocTypes;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, RulePreparation, SemanticModel, Truth,
};
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
    let mut events = Vec::new();
    if matches!(passing, Passing::ContextToBloc)
        && let Some(identities) = ctx.identities
    {
        let model = SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types);
        for declaration in program.declarations() {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            let Some(supertype) = class.extends() else {
                continue;
            };
            let ty = model.resolve_type(supertype);
            if types.is_api_type(&ty, &model, "Bloc")
                && let Some(ResolvedType::Interface { identity, .. }) = ty.arguments().first()
                && matches!(identity, DeclarationIdentity::Project { .. })
                && !events.contains(identity)
            {
                events.push(identity.clone());
            }
        }
    }
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
    events: Vec<DeclarationIdentity>,
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
                && self.ctx.identities.is_some_and(|index| {
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
            && let Expr::Field(field) = call.callee().unparenthesized()
        {
            (state.infer(field.object()), call.args())
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
        let matches_target = match self.passing {
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
                    || self.events.iter().any(|event| {
                        state.signatures.is_subtype_of(target, event, &state.model) == Truth::Yes
                    })
            }
        };
        if !matches_target {
            return;
        }
        for argument in args
            .positional()
            .chain(args.named().map(|argument| argument.value()))
        {
            let ty = state.infer(argument);
            let matches_argument = match self.passing {
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
            };
            if matches_argument {
                let (rule, message) = match self.passing {
                    Passing::BlocToBloc => (
                        "avoid-passing-bloc-to-bloc",
                        "Avoid coupling blocs by passing a Bloc or Cubit to another bloc.",
                    ),
                    Passing::ContextToBloc => (
                        "avoid-passing-build-context-to-blocs",
                        "Keep Flutter BuildContext out of Bloc and Cubit dependencies.",
                    ),
                    Passing::BlocToWidget => (
                        "avoid-passing-bloc-to-widget",
                        "Obtain the Bloc or Cubit from the widget context instead of passing it directly.",
                    ),
                };
                self.diagnostics
                    .push(self.ctx.diagnostic(rule, message, argument.span()));
            }
        }
    }
}
