//! Flags confirmed hook widgets and `HookBuilder` callbacks that do not use hooks.
//!
//! Replace an unused hook widget with a regular widget to simplify its lifecycle.
//! Only concrete, directly package-derived `HookWidget` and `HookConsumerWidget`
//! classes with their own build body, and inline `HookBuilder` callbacks, are
//! checked. Every call except a generative constructor of a confirmed Flutter
//! `Widget` subtype retains its immediate hook owner. Getter reads and cascades
//! also retain that owner; nested callbacks do not retain an outer owner.
//! Abstract classes, inherited builds, and unresolved hook widget identities are
//! skipped. Classes extended elsewhere in the same library are skipped; subclasses
//! in other libraries are not visible and must be checked before conversion.
//! Indirect hooks through overloaded operators or setters are not tracked.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, ResolvedType, Rule, RulePreparation,
    SemanticModel, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::scoped_constructor_invocation;
use crate::lint::flutter_hooks::is_hook_widget;
use crate::lint::function_bodies::FunctionBodies;

#[derive(Default)]
pub struct AvoidUnnecessaryHookWidgets;

impl Rule for AvoidUnnecessaryHookWidgets {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let mut candidates = Vec::new();
        for declaration in program.declarations() {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            if class.modifiers().is_abstract {
                continue;
            }
            let Some(supertype) = class.extends() else {
                continue;
            };
            let ResolvedType::Interface { identity, .. } = model.resolve_type(supertype) else {
                continue;
            };
            if !is_hook_widget(&identity) {
                continue;
            }
            let Some(class_identity) = model.resolve_name(class.name().name()) else {
                continue;
            };
            let programs = std::iter::once(program).chain(
                ctx.library()
                    .into_iter()
                    .flat_map(|library| library.siblings().iter().copied()),
            );
            if programs
                .flat_map(|program| program.declarations())
                .any(|declaration| {
                    let superclass = match declaration {
                        TopLevelDecl::Class(class) => class.extends(),
                        TopLevelDecl::ClassTypeAlias(alias) => Some(alias.superclass()),
                        _ => None,
                    };
                    superclass.is_some_and(|superclass| {
                        matches!(model.resolve_type(superclass),
                        ResolvedType::Interface { identity, .. } if identity == class_identity)
                    })
                })
            {
                continue;
            }
            for member in class.members() {
                if let ClassMember::Method(method) = member
                    && method.name().name() == "build"
                    && !method.is_static()
                    && let Some(body @ (FunctionBody::Block(_) | FunctionBody::Arrow(_))) =
                        method.body()
                {
                    candidates.push(Candidate {
                        body: body.span(),
                        report: class.name().span(),
                        needed: false,
                    });
                }
            }
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            candidates,
            bodies: FunctionBodies::new(program),
            widget: DeclarationIdentity::flutter("Widget"),
        }))
    }
}

struct Candidate {
    body: Span,
    report: Span,
    needed: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    candidates: Vec<Candidate>,
    bodies: FunctionBodies<'a>,
    widget: DeclarationIdentity,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let construction = if matches!(expression, Expr::Call(_) | Expr::New(_)) {
            scoped_constructor_invocation(expression, state).filter(|(identity, name, _)| {
                state.signatures.has_constructor(identity, name)
                    && !matches!(expression, Expr::Call(call)
                        if falcon_analyze::expression_segments(call.callee(), true)
                            .and_then(|segments| segments.first().copied())
                            .is_none_or(|head| {
                                state.implicit_member_type(head).is_some()
                                    || matches!(&state.enclosing_type,
                                        Some(ResolvedType::Interface { identity, .. })
                                            if !state.signatures.declared_member_facts(identity, head).is_empty())
                            }))
            })
        } else {
            None
        };
        if let Some((identity, _, arguments)) = &construction
            && identity.is_package("flutter_hooks", "HookBuilder")
            && let Some(argument) = arguments
                .named()
                .find(|argument| argument.name().name() == "builder")
            && let Expr::FuncExpr(callback) = argument.value().unparenthesized()
        {
            self.candidates.push(Candidate {
                body: callback.body().span(),
                report: crate::lint::flutter::constructor_span(expression),
                needed: false,
            });
        }
        if self.candidates.is_empty() {
            return;
        }
        let Some(body) = self.bodies.enclosing(&expression.span()) else {
            return;
        };
        let might_use_hooks = match expression {
            Expr::Call(_) | Expr::New(_) => construction.is_none_or(|(identity, name, _)| {
                if state.signatures.declaration(&identity).is_none_or(|facts| {
                    facts
                        .constructors
                        .iter()
                        .any(|constructor| constructor.name == name && constructor.is_factory)
                }) {
                    return true;
                }
                state.signatures.is_subtype_of(
                    &ResolvedType::interface_identity(identity),
                    &self.widget,
                    &state.model,
                ) != Truth::Yes
            }),
            Expr::Cascade(_) => true,
            _ => getter_read(expression, state),
        };
        if might_use_hooks
            && let Some(candidate) = self
                .candidates
                .iter_mut()
                .find(|candidate| candidate.body == body.span)
        {
            candidate.needed = true;
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates
            .into_iter()
            .filter(|candidate| !candidate.needed)
            .map(|candidate| {
                self.ctx.diagnostic("This widget does not use hooks; replace it with a regular widget.", candidate.report)
            })
            .collect()
    }
}

fn getter_read<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    match expression {
        Expr::Ident(identifier) if !state.environment.is_bound(identifier.name()) => {
            state
                .model
                .resolve_value(identifier.name())
                .is_some_and(|identity| state.signatures.is_value_getter(&identity))
                || state.enclosing_type.as_ref().is_some_and(|receiver| {
                    state
                        .signatures
                        .resolved_member_facts(receiver, identifier.name(), &state.model)
                        .is_some_and(|facts| {
                            facts.iter().any(|fact| fact.kind == MemberKind::Getter)
                        })
                })
        }
        Expr::Field(field) => {
            let receiver = state.infer_receiver(field.object());
            let name = field.field().name();
            state
                .signatures
                .resolved_member_facts(&receiver, name, &state.model)
                .is_some_and(|facts| facts.iter().any(|fact| fact.kind == MemberKind::Getter))
                || state
                    .signatures
                    .resolved_extension_getter(&receiver, name, &state.model)
                    .is_some()
                || falcon_analyze::expression_segments(field.object(), true)
                    .and_then(|segments| state.model.resolve_dotted(&segments))
                    .is_some_and(|identity| {
                        state
                            .signatures
                            .declared_member_facts(&identity, name)
                            .iter()
                            .any(|fact| fact.kind == MemberKind::Getter)
                    })
                || matches!(field.object(), Expr::Ident(prefix)
                    if !state.environment.is_bound(prefix.name())
                        && state.model.resolve_value(falcon_analyze::QualifiedName::prefixed(prefix.name(), name))
                            .is_some_and(|identity| state.signatures.is_value_getter(&identity)))
        }
        _ => false,
    }
}
