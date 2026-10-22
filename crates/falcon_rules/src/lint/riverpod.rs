//! Shared package identity and build-body queries for Riverpod rules.

use crate::lint::flutter::scoped_constructor_invocation;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, RulePreparation, Truth};
use falcon_analyze::{BodyOwner, FunctionBodies};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{CascadeOp, ClassMember, Expr, Program, Span, TopLevelDecl};

const NOTIFIERS: &[&str] = &[
    "Notifier",
    "NotifierBase",
    "BuildlessNotifier",
    "AutoDisposeNotifier",
    "AsyncNotifier",
    "AsyncNotifierBase",
    "BuildlessAsyncNotifier",
    "AutoDisposeAsyncNotifier",
    "StreamNotifier",
    "StreamNotifierBase",
    "AutoDisposeStreamNotifier",
    "$Notifier",
    "$AsyncNotifier",
    "$StreamNotifier",
    "StateNotifier",
];

pub(crate) fn is_type(
    ctx: &AnalyzeContext<'_>,
    state: &SemanticState<'_>,
    ty: &ResolvedType,
    names: &[&str],
) -> bool {
    let ResolvedType::Interface { identity, .. } = ty else {
        return false;
    };
    is_identity(ctx, identity, names)
        || state
            .signatures
            .supertype_matches(identity, &|identity| is_identity(ctx, identity, names))
            == Truth::Yes
}

pub(crate) fn is_exact_type(ctx: &AnalyzeContext<'_>, ty: &ResolvedType, names: &[&str]) -> bool {
    matches!(ty, ResolvedType::Interface { identity, .. } if is_identity(ctx, identity, names))
}

pub(crate) fn is_identity(
    ctx: &AnalyzeContext<'_>,
    identity: &DeclarationIdentity,
    names: &[&str],
) -> bool {
    ["riverpod", "flutter_riverpod", "hooks_riverpod"]
        .iter()
        .any(|package| {
            names.iter().any(|name| {
                ctx.identities().is_some_and(|identities| {
                    identities.is_package_declaration(identity, package, name)
                })
            })
        })
        || names.contains(&"StateNotifier")
            && ctx.identities().is_some_and(|identities| {
                identities.is_package_declaration(identity, "state_notifier", "StateNotifier")
            })
}

fn is_build_callback(
    ctx: &AnalyzeContext<'_>,
    identity: &DeclarationIdentity,
    constructor: &str,
) -> bool {
    let Some(identities) = ctx.identities() else {
        return false;
    };
    if identities.is_package_declaration(identity, "flutter_hooks", "HookBuilder") {
        return true;
    }
    identities.is_package_declaration(identity, "flutter", identity.name())
        && (matches!(
            identity.name(),
            "Builder"
                | "LayoutBuilder"
                | "StatefulBuilder"
                | "ValueListenableBuilder"
                | "ListenableBuilder"
                | "AnimatedBuilder"
                | "FutureBuilder"
                | "StreamBuilder"
                | "TweenAnimationBuilder"
                | "OrientationBuilder"
                | "SliverChildBuilderDelegate"
                | "AnimatedList"
                | "SliverAnimatedList"
                | "ReorderableList"
                | "SliverReorderableList"
                | "ListWheelChildBuilderDelegate"
        ) || matches!(
            identity.name(),
            "ListView"
                | "GridView"
                | "PageView"
                | "SliverList"
                | "SliverGrid"
                | "ReorderableListView"
        ) && matches!(constructor, "builder" | "separated"))
}

#[derive(Clone, Copy)]
pub(crate) enum BuildRule {
    WatchOutside,
    ReadInside,
    ReadForMethods,
    NotifierMembers,
}

pub(crate) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    rule: BuildRule,
) -> RulePreparation<'a> {
    let mut builds = Vec::new();
    for declaration in program
        .declarations()
        .filter(|_| !matches!(rule, BuildRule::ReadForMethods))
    {
        let members = match declaration {
            TopLevelDecl::Class(class) => class.members(),
            TopLevelDecl::MixinClass(class) => class.members(),
            _ => continue,
        };
        for member in members {
            if let ClassMember::Method(method) = member
                && method.name().name() == "build"
                && !method.is_static()
                && let Some(body) = method.body()
            {
                builds.push(body.span());
            }
        }
    }
    RulePreparation::Semantic(Box::new(BuildCollector {
        ctx,
        rule,
        builds,
        consumer_builds: Vec::new(),
        bodies: (!matches!(rule, BuildRule::ReadForMethods)).then(|| ctx.function_bodies(program)),
        diagnostics: Vec::new(),
    }))
}

struct BuildCollector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    rule: BuildRule,
    builds: Vec<Span>,
    consumer_builds: Vec<Span>,
    bodies: Option<&'a FunctionBodies<'a>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> BuildCollector<'a> {
    fn inside_build(&self, expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
        self.bodies
            .as_ref()
            .and_then(|bodies| bodies.enclosing(&expression.span()))
            .is_some_and(|body| {
                self.consumer_builds.contains(&body.span)
                    || body.owner == BodyOwner::Method
                        && self.builds.iter().any(|span| span.contains(body.span))
                        && state.enclosing_type.as_ref().is_some_and(|ty| {
                            is_type(
                                self.ctx,
                                state,
                                ty,
                                &["ConsumerWidget", "HookConsumerWidget", "ConsumerState"],
                            )
                        })
            })
    }

    fn check_method(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Field(member) = expression else {
            return;
        };
        if matches!(member.field().name(), "toString" | "noSuchMethod") {
            return;
        }
        let Expr::Call(watch) = member.object().unparenthesized() else {
            return;
        };
        let Expr::Field(watch_member) = watch.callee().unparenthesized() else {
            return;
        };
        if watch_member.field().name() == "watch"
            && is_type(
                self.ctx,
                state,
                &state.infer(watch_member.object()),
                &["Ref", "WidgetRef"],
            )
            && is_type(self.ctx, state, &state.infer(member.object()), NOTIFIERS)
            && matches!(state.infer(expression), ResolvedType::Function { .. })
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "Use `ref.read` rather than `ref.watch` when invoking notifier methods.",
                member.field().span(),
            ));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for BuildCollector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(self.rule, BuildRule::NotifierMembers)
            && let Expr::Cascade(cascade) = expression
            && self.inside_build(expression, state)
            && is_type(self.ctx, state, &state.infer(cascade.object()), NOTIFIERS)
        {
            for section in cascade.sections() {
                if let Some(CascadeOp::Call(call)) = section.ops().next()
                    && !matches!(call.name().name(), "toString" | "noSuchMethod")
                {
                    self.diagnostics.push(self.ctx.diagnostic("Do not invoke notifier methods during build; move the action to a callback.", call.name().span()));
                }
            }
        }
        if matches!(self.rule, BuildRule::ReadForMethods) {
            self.check_method(expression, state);
            return;
        }
        if let Some((identity, constructor, args)) =
            scoped_constructor_invocation(expression, state)
            && (is_identity(self.ctx, &identity, &["Consumer", "HookConsumer"])
                || self.inside_build(expression, state)
                    && is_build_callback(self.ctx, &identity, constructor))
        {
            if identity.name() == "SliverChildBuilderDelegate"
                && let Some(argument) = args.positional().next()
                && let Expr::FuncExpr(function) = argument.unparenthesized()
            {
                self.consumer_builds.push(function.body().span());
            }
            for argument in args.named() {
                if matches!(
                    argument.name().name(),
                    "builder" | "itemBuilder" | "separatorBuilder"
                ) && let Expr::FuncExpr(function) = argument.value().unparenthesized()
                {
                    self.consumer_builds.push(function.body().span());
                }
            }
        }
        let Expr::Call(call) = expression else {
            return;
        };
        let Expr::Field(member) = call.callee().unparenthesized() else {
            return;
        };
        let method = member.field().name();
        if matches!(self.rule, BuildRule::WatchOutside) && method != "watch"
            || matches!(self.rule, BuildRule::ReadInside) && method != "read"
            || matches!(self.rule, BuildRule::NotifierMembers)
                && matches!(method, "toString" | "noSuchMethod")
        {
            return;
        }
        let inside_build = self.inside_build(expression, state);
        let (message, report) = match self.rule {
            BuildRule::WatchOutside => (
                "Use `ref.read` outside build instead of subscribing with `ref.watch`.",
                !inside_build && is_type(self.ctx, state, &state.infer(member.object()), &["WidgetRef"])
            ),
            BuildRule::ReadInside => (
                "Use `ref.watch` in build so provider changes rebuild the widget.",
                inside_build
                    && is_type(self.ctx, state, &state.infer(member.object()), &["Ref", "WidgetRef"])
                    && !call.args().positional().any(|arg| matches!(arg.unparenthesized(), Expr::Field(field) if field.field().name() == "notifier"))
            ),
            BuildRule::ReadForMethods => return,
            BuildRule::NotifierMembers => (
                "Do not invoke notifier methods during build; move the action to a callback.",
                inside_build && is_type(self.ctx, state, &state.infer(member.object()), NOTIFIERS)
            ),
        };
        if report {
            self.diagnostics
                .push(self.ctx.diagnostic(message, member.field().span()));
        }
    }
}
