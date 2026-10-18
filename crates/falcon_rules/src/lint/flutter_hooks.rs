//! Resolved Flutter Hooks identities and executable contexts shared by hook rules.

use std::collections::HashSet;

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{DeclarationIdentity, QualifiedName, ResolvedType};
use falcon_syntax::cst::*;

use crate::lint::function_bodies::FunctionBodies;

pub(crate) fn hook_name(name: &str) -> bool {
    name.strip_prefix('_')
        .unwrap_or(name)
        .strip_prefix("use")
        .and_then(|suffix| suffix.chars().next())
        .is_some_and(|letter| letter.is_ascii_uppercase())
}

pub(crate) fn is_hook_widget(identity: &DeclarationIdentity) -> bool {
    identity.is_package("flutter_hooks", "HookWidget")
        || identity.is_package("hooks_riverpod", "HookConsumerWidget")
}

pub(crate) fn is_hook_builder_context(expr: Expr<'_>, state: &SemanticState<'_>) -> bool {
    crate::lint::flutter::scoped_constructor_invocation(expr, state).is_some_and(
        |(identity, _, _)| {
            identity.is_package("flutter_hooks", "HookBuilder")
                || identity.is_package("hooks_riverpod", "HookConsumer")
        },
    )
}

fn hook_enabled_widget(identity: &DeclarationIdentity) -> bool {
    is_hook_widget(identity)
        || identity.is_package("flutter_hooks", "StatefulHookWidget")
        || identity.is_package("hooks_riverpod", "StatefulHookConsumerWidget")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WidgetContext {
    Hook,
    NonHook,
    NonWidget,
    Unknown,
}

fn widget_kind(ty: &ResolvedType, state: &SemanticState<'_>) -> WidgetContext {
    let mut current = ty;
    let mut visited = HashSet::new();
    loop {
        let ResolvedType::Interface { identity, .. } = current else {
            return WidgetContext::Unknown;
        };
        if !visited.insert(identity) {
            return WidgetContext::Unknown;
        }
        if hook_enabled_widget(identity) {
            return WidgetContext::Hook;
        }
        if ["StatelessWidget", "StatefulWidget", "Widget"]
            .iter()
            .any(|name| identity.is_package("flutter", name))
        {
            return WidgetContext::NonHook;
        }
        if *identity == DeclarationIdentity::OBJECT {
            return WidgetContext::NonWidget;
        }
        match state.signatures.superclass(identity) {
            Some(parent) => current = parent,
            None if matches!(identity, DeclarationIdentity::Project { .. })
                && matches!(
                    state.signatures.type_kind(identity),
                    Some(TypeKind::Class | TypeKind::MixinClass | TypeKind::Enum)
                ) =>
            {
                return WidgetContext::NonWidget;
            }
            None => return WidgetContext::Unknown,
        }
    }
}

fn widget_context(state: &SemanticState<'_>) -> WidgetContext {
    let Some(ty) = &state.enclosing_type else {
        return WidgetContext::Unknown;
    };
    if let Some(ResolvedType::Interface { arguments, .. }) =
        state
            .signatures
            .as_instance_of(ty, &DeclarationIdentity::flutter("State"), &state.model)
    {
        return match arguments.first().map(|widget| widget_kind(widget, state)) {
            Some(WidgetContext::Hook) => WidgetContext::Hook,
            Some(WidgetContext::NonHook) => WidgetContext::NonHook,
            _ => WidgetContext::Unknown,
        };
    }
    widget_kind(ty, state)
}

pub(crate) fn hook_widget_context(state: &SemanticState<'_>) -> bool {
    widget_context(state) == WidgetContext::Hook
}

pub(crate) fn non_widget_context(state: &SemanticState<'_>) -> bool {
    widget_context(state) == WidgetContext::NonWidget
}

fn package_hook(identity: &DeclarationIdentity, state: &SemanticState<'_>) -> bool {
    (identity.name() == "use" || hook_name(identity.name()))
        && (identity.is_package("flutter_hooks", identity.name())
            || state
                .model
                .declaration_uri(identity)
                .is_some_and(|uri| uri.starts_with("package:flutter_hooks/")))
}

pub(crate) fn call_identity(
    expr: Expr<'_>,
    state: &SemanticState<'_>,
) -> Option<DeclarationIdentity> {
    let Expr::Call(call) = expr.unparenthesized() else {
        return None;
    };
    let callee = call.callee().unparenthesized();
    let callee = match callee {
        Expr::GenericInstantiation(node) => node.target().unparenthesized(),
        other => other,
    };
    let (head, name) = match callee {
        Expr::Ident(name) => (name.name(), QualifiedName::from(name.name())),
        Expr::Field(field) if !field.is_null_safe() => {
            let Expr::Ident(prefix) = field.object().unparenthesized() else {
                return None;
            };
            (
                prefix.name(),
                QualifiedName::prefixed(prefix.name(), field.field().name()),
            )
        }
        _ => return None,
    };
    if state.environment.is_bound(head)
        || state.type_parameters.contains(head)
        || state.enclosing_type.as_ref().is_some_and(|ty| {
            matches!(ty, ResolvedType::Interface { identity, .. }
            if !state.signatures.declared_member_facts(identity, head).is_empty()
                || state.signatures.member(identity, head).is_some()
                || state.signatures.field(identity, head).is_some())
        })
    {
        return None;
    }
    state.model.resolve_value(name)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct HookTarget<'a> {
    owner: DeclarationIdentity,
    member: Option<&'a str>,
}

struct CustomHook<'a> {
    name: &'a str,
    class: Option<&'a str>,
    body: Span,
    target: Option<HookTarget<'a>>,
}

struct Call<'a> {
    span: (usize, usize),
    body: Option<Span>,
    target: Option<HookTarget<'a>>,
    hook: bool,
}

/// Records semantic calls, then computes the least fixed point of composing
/// custom hooks. A useX name alone never establishes hook identity.
pub(crate) struct HookCalls<'a> {
    bodies: FunctionBodies<'a>,
    custom: Vec<CustomHook<'a>>,
    calls: Vec<Call<'a>>,
    hooks: HashSet<(usize, usize)>,
}
impl<'a> HookCalls<'a> {
    pub(crate) fn new(program: Program<'a>) -> Self {
        let mut custom = Vec::new();
        for declaration in program.declarations() {
            match declaration {
                TopLevelDecl::Function(function) if hook_name(function.name().name()) => {
                    if let Some(body) = function.body() {
                        custom.push(CustomHook {
                            name: function.name().name(),
                            class: None,
                            body: body.span(),
                            target: None,
                        });
                    }
                }
                declaration if crate::lint::declarations::members_of(declaration).is_some() => {
                    let owner = match declaration {
                        TopLevelDecl::Class(node) => Some(node.name().name()),
                        TopLevelDecl::Mixin(node) => Some(node.name().name()),
                        TopLevelDecl::MixinClass(node) => Some(node.name().name()),
                        TopLevelDecl::Enum(node) => Some(node.name().name()),
                        TopLevelDecl::ExtensionType(node) => Some(node.name().name()),
                        _ => None,
                    };
                    let Some(owner) = owner else { continue };
                    for member in crate::lint::declarations::members_of(declaration)
                        .into_iter()
                        .flatten()
                    {
                        if let ClassMember::Method(method) = member
                            && hook_name(method.name().name())
                            && let Some(body) = method.body()
                        {
                            custom.push(CustomHook {
                                name: method.name().name(),
                                class: Some(owner),
                                body: body.span(),
                                target: None,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        Self {
            bodies: FunctionBodies::new(program),
            custom,
            calls: Vec::new(),
            hooks: HashSet::new(),
        }
    }

    pub(crate) fn observe(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expr.unparenthesized() else {
            return;
        };
        let callee = match call.callee().unparenthesized() {
            Expr::GenericInstantiation(node) => node.target().unparenthesized(),
            other => other,
        };
        let name = match callee {
            Expr::Ident(name) => name.name(),
            Expr::Field(field) => field.field().name(),
            _ => return,
        };
        if name != "use" && !hook_name(name) {
            return;
        }
        let body = self.bodies.enclosing(&expr.span()).map(|body| body.span);
        if let Some(function) = self
            .custom
            .iter_mut()
            .find(|function| Some(function.body) == body)
            && function.target.is_none()
        {
            function.target = match function.class {
                Some(class) => state.model.resolve_name(class).map(|owner| HookTarget {
                    owner,
                    member: Some(function.name),
                }),
                None => state
                    .model
                    .resolve_value(function.name)
                    .map(|owner| HookTarget {
                        owner,
                        member: None,
                    }),
            };
        }
        let identity = call_identity(expr, state);
        let hook = identity
            .as_ref()
            .is_some_and(|identity| package_hook(identity, state));
        let target = identity
            .map(|owner| HookTarget {
                owner,
                member: None,
            })
            .or_else(|| {
                let Expr::Ident(name) = callee else {
                    return None;
                };
                if !hook_name(name.name()) || state.environment.is_bound(name.name()) {
                    return None;
                }
                let ResolvedType::Interface { identity, .. } = state.enclosing_type.as_ref()?
                else {
                    return None;
                };
                state.signatures.member(identity, name.name())?;
                Some(HookTarget {
                    owner: identity.clone(),
                    member: Some(name.name()),
                })
            });
        self.calls.push(Call {
            span: (expr.span().start, expr.span().end),
            body,
            target,
            hook,
        });
    }

    pub(crate) fn finish(&mut self) {
        let mut custom_hooks = HashSet::with_capacity(self.custom.len());
        loop {
            let mut changed = false;
            for function in &self.custom {
                if let Some(target) = &function.target
                    && !custom_hooks.contains(target)
                    && self.calls.iter().any(|call| {
                        call.body == Some(function.body)
                            && (call.hook
                                || call
                                    .target
                                    .as_ref()
                                    .is_some_and(|target| custom_hooks.contains(target)))
                    })
                {
                    custom_hooks.insert(target.clone());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        self.hooks.extend(
            self.calls
                .iter()
                .filter(|call| {
                    call.hook
                        || call
                            .target
                            .as_ref()
                            .is_some_and(|target| custom_hooks.contains(target))
                })
                .map(|call| call.span),
        );
    }

    pub(crate) fn contains(&self, expr: Expr<'_>) -> bool {
        matches!(expr.unparenthesized(), Expr::Call(_))
            && self.hooks.contains(&(expr.span().start, expr.span().end))
    }
}

/// True only for confirmed Flutter widget/State ancestry, excluding hook widgets.
pub(crate) fn non_hook_widget(state: &SemanticState<'_>) -> bool {
    widget_context(state) == WidgetContext::NonHook
}
