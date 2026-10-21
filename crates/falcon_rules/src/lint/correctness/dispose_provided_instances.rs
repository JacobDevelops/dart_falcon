//! Flags provider-created instances with a dispose method that lack ref.onDispose cleanup.
//!
//! Riverpod autoDispose destroys provider state, not arbitrary disposable objects.
//! Register each owned instance's dispose method with the provider's ref.onDispose.
//! Only package-confirmed Provider/FutureProvider/StreamProvider callbacks and
//! resolved constructor-created resources are checked. Nested deferred closures,
//! borrowed values, unknown types, and types without a recognized release method
//! are excluded. Direct cleanup and named local callbacks are recognized; opaque
//! cleanup calls mentioning the resource conservatively count as registration.
//! Borrowed factory returns are recognized only when their simple body is visible
//! in this file; constructor metadata does not expose bodies from other libraries.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct DisposeProvidedInstances;

impl Rule for DisposeProvidedInstances {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare_resources(program, ctx, true)
    }
}

pub(super) fn prepare_resources<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    provided_instances: bool,
) -> RulePreparation<'a> {
    let enabled = ctx.identities().is_some()
        && program
            .imports()
            .any(|import| riverpod_uri(&import.uri().value()));
    RulePreparation::Semantic(Box::new(Collector {
        ctx,
        program,
        provided_instances,
        enabled,
        providers: Vec::new(),
        functions: Vec::new(),
        blocks: Vec::new(),
        returns: Vec::new(),
        resources: Vec::new(),
    }))
}

struct Provider<'a> {
    body: FunctionBody<'a>,
    auto_dispose: bool,
    reference: &'a str,
}

struct Resource<'a> {
    provider: usize,
    name: Option<&'a str>,
    method: &'static str,
    span: Span,
    shipped_leak: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    provided_instances: bool,
    enabled: bool,
    providers: Vec<Provider<'a>>,
    functions: Vec<Span>,
    blocks: Vec<CleanupScope<'a>>,
    resources: Vec<Resource<'a>>,
    returns: Vec<(usize, &'a str)>,
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn riverpod_uri(uri: &str) -> bool {
    [
        "package:riverpod/",
        "package:flutter_riverpod/",
        "package:hooks_riverpod/",
    ]
    .iter()
    .any(|package| uri.starts_with(package))
}

#[derive(Clone, Copy)]
enum CleanupScope<'a> {
    Block(Block<'a>),
    Case(SwitchCase<'a>),
}

impl<'a> CleanupScope<'a> {
    fn span(self) -> Span {
        match self {
            Self::Block(block) => block.span(),
            Self::Case(case) => case.span(),
        }
    }

    fn statements(self) -> AstChildren<'a, Stmt<'a>> {
        match self {
            Self::Block(block) => block.stmts(),
            Self::Case(case) => case.body(),
        }
    }
}

impl<'a> Collector<'a> {
    fn provider_at(&self, span: Span) -> Option<usize> {
        if self.providers.is_empty() {
            return None;
        }
        let function = self
            .functions
            .iter()
            .rev()
            .find(|body| contains(**body, span))?;
        self.providers
            .iter()
            .rposition(|provider| provider.body.span() == *function)
    }

    fn resource(
        &mut self,
        expression: Expr<'a>,
        name: Option<&'a str>,
        span: Span,
        state: &SemanticState<'a>,
    ) {
        if self.providers.is_empty() {
            return;
        }
        let Some(provider) = self.provider_at(span) else {
            return;
        };
        let Some((identity, constructor, _)) =
            crate::lint::flutter::scoped_constructor_invocation(expression, state)
        else {
            return;
        };
        if !state.signatures.has_constructor(&identity, constructor) {
            return;
        }
        if state
            .signatures
            .declaration(&identity)
            .is_some_and(|facts| {
                facts.constructors.iter().any(|facts| {
                    facts.name == constructor && facts.is_factory && facts.redirect_target.is_none()
                })
            })
            && borrowed_factory(self.program, &identity, constructor, state)
        {
            return;
        }
        let ty = falcon_analyze::ResolvedType::interface_identity(identity);
        let methods: &[&str] = if self.provided_instances {
            &["dispose"]
        } else {
            &["close", "cancel"]
        };
        let Some(method) = methods.iter().copied().find(|method| {
            state
                .signatures
                .resolved_member(&ty, method, &state.model)
                .is_some_and(|(signature, _)| {
                    signature.positional.len() == signature.optional_positional
                        && signature
                            .shape
                            .as_ref()
                            .is_some_and(|shape| shape.required_named.is_empty())
                })
        }) else {
            return;
        };
        self.resources.push(Resource {
            provider,
            name,
            method,
            span,
            shipped_leak: !self.provided_instances
                && [
                    ("dart:core", "Sink"),
                    ("dart:io", "Socket"),
                    ("dart:async", "StreamSubscription"),
                ]
                .iter()
                .any(|(library, name)| {
                    state
                        .signatures
                        .interface_check_sdk(&ty, library, name, &state.model)
                        == falcon_analyze::Truth::Yes
                }),
        });
    }

    fn provider(&self, call: CallExpr<'a>, state: &SemanticState<'a>) -> Option<Provider<'a>> {
        let segments = falcon_analyze::expression_segments(call.callee(), true)?;
        let index = segments
            .iter()
            .position(|name| matches!(*name, "Provider" | "FutureProvider" | "StreamProvider"))?;
        if index > 1
            || segments[index + 1..]
                .iter()
                .any(|name| !matches!(*name, "autoDispose" | "family"))
        {
            return None;
        }
        let prefix = (index == 1).then(|| segments[0]);
        if state.environment.is_bound(segments[0]) {
            return None;
        }
        let name = segments[index];
        if !self.program.imports().any(|import| {
            riverpod_uri(&import.uri().value())
                && import.as_name().map(|name| name.name()) == prefix
                && import.combinators().all(|combinator| match combinator {
                    ImportCombinator::Show(show) => show.names().any(|item| item.name() == name),
                    ImportCombinator::Hide(hide) => hide.names().all(|item| item.name() != name),
                })
        }) {
            return None;
        }
        let qualified = falcon_analyze::QualifiedName { prefix, name };
        if let Some(identity) = state.model.resolve_name(qualified)
            && !["riverpod", "flutter_riverpod", "hooks_riverpod"]
                .iter()
                .any(|package| {
                    self.ctx.identities().is_some_and(|identities| {
                        identities.is_package_declaration(&identity, package, identity.name())
                    })
                })
        {
            return None;
        }
        let Expr::FuncExpr(callback) = call.args().positional().next()?.unparenthesized() else {
            return None;
        };
        let reference = callback.params().params().next()?.name().name();
        Some(Provider {
            body: callback.body(),
            auto_dispose: segments.contains(&"autoDispose"),
            reference,
        })
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if !self.enabled {
            return;
        }
        match node {
            Expr::Call(call) => {
                if let Some(provider) = self.provider(call, state) {
                    self.providers.push(provider);
                }
            }
            Expr::FuncExpr(function) => {
                self.functions.push(function.body().span());
                if let FunctionBody::Arrow(body) = function.body() {
                    self.resource(body.expr(), None, body.expr().span(), state);
                }
            }
            _ => {}
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if !self.enabled {
            return;
        }
        match node {
            Stmt::Switch(statement) if self.provider_at(statement.span()).is_some() => {
                self.blocks
                    .extend(statement.cases().map(CleanupScope::Case));
            }
            Stmt::LocalFunc(function) => self.functions.push(function.body().span()),
            Stmt::LocalVar(declaration) => {
                for variable in declaration.declarators() {
                    if let Some(initializer) = variable.initializer() {
                        self.resource(
                            initializer,
                            Some(variable.name().name()),
                            variable.name().span(),
                            state,
                        );
                    }
                }
            }
            Stmt::Return(statement) => {
                if let Some(expression) = statement.value() {
                    self.resource(expression, None, expression.span(), state);
                    if let Expr::Ident(identifier) = expression.unparenthesized()
                        && let Some(provider) = self.provider_at(expression.span())
                    {
                        self.returns.push((provider, identifier.name()));
                    }
                }
            }
            _ => {}
        }
    }
    fn visit_block_end(&mut self, node: Block<'a>, _state: &SemanticState<'a>) {
        if !self.providers.is_empty() && self.provider_at(node.span()).is_some() {
            self.blocks.push(CleanupScope::Block(node));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.resources.iter().filter_map(|resource| {
            let provider = &self.providers[resource.provider];
            if resource.shipped_leak && resource.name.is_some_and(|name| {
                !self.returns.iter().any(|&(provider, returned)| provider == resource.provider && returned == name)
            }) {
                return None;
            }
            let cleaned_up = resource.name.is_some_and(|name| {
                let Some(block) = self.blocks.iter().filter(|block| contains(block.span(), resource.span))
                    .min_by_key(|block| block.span().end - block.span().start) else {
                    return false;
                };
                if self.provided_instances {
                    registered(provider, *block, name, resource.method)
                } else {
                    ["dispose", "close", "cancel"].iter().any(|method| registered(provider, *block, name, method))
                }
            });
            if (!self.provided_instances && provider.auto_dispose) || cleaned_up
            {
                return None;
            }
            let message = if self.provided_instances {
                "Register this instance's dispose method with ref.onDispose; autoDispose does not dispose the instance."
            } else {
                "Give this provider-created resource a lifecycle: use autoDispose or register its cleanup with ref.onDispose."
            };
            Some(self.ctx.diagnostic(message, resource.span))
        }).collect()
    }
}

fn registered(provider: &Provider<'_>, scope: CleanupScope<'_>, name: &str, method: &str) -> bool {
    scope.statements().any(|statement| match statement {
        Stmt::Expr(statement) => registration(statement.expr(), provider, scope, name, method),
        Stmt::LocalVar(declaration) => declaration.declarators().any(|variable| {
            variable
                .initializer()
                .is_some_and(|expression| registration(expression, provider, scope, name, method))
        }),
        _ => false,
    })
}

fn registration(
    expression: Expr<'_>,
    provider: &Provider<'_>,
    scope: CleanupScope<'_>,
    name: &str,
    method: &str,
) -> bool {
    match expression.unparenthesized() {
        Expr::Call(call) => {
            let Expr::Field(field) = call.callee().unparenthesized() else { return false };
            field.field().name() == "onDispose"
                && matches!(field.object().unparenthesized(), Expr::Ident(reference) if reference.name() == provider.reference)
                && call.args().positional().next().is_some_and(|callback| cleanup(callback, scope, name, method))
        }
        Expr::Cascade(cascade) if matches!(cascade.object().unparenthesized(), Expr::Ident(reference) if reference.name() == provider.reference) => {
            cascade.sections().any(|section| {
                matches!(section.ops().next(), Some(CascadeOp::Call(call))
                    if call.name().name() == "onDispose"
                        && call.args().positional().next().is_some_and(|callback| cleanup(callback, scope, name, method)))
            })
        }
        _ => false,
    }
}

fn cleanup(callback: Expr<'_>, scope: CleanupScope<'_>, name: &str, method: &str) -> bool {
    match callback.unparenthesized() {
        Expr::Field(field) => release_target(field, name, method),
        Expr::Ident(identifier) => scope.statements().any(|statement| match statement {
            Stmt::LocalFunc(function) => function.name().name() == identifier.name()
                && cleanup_body(function.body(), name, method),
            Stmt::LocalVar(declaration) => declaration.declarators().any(|variable| {
                variable.name().name() == identifier.name()
                    && matches!(variable.initializer().map(Expr::unparenthesized), Some(Expr::FuncExpr(function))
                        if function.params().params().next().is_none()
                            && cleanup_body(function.body(), name, method))
            }),
            _ => false,
        }),
        Expr::FuncExpr(function) if function.params().params().next().is_none() => {
            cleanup_body(function.body(), name, method)
        }
        _ => false,
    }
}

fn cleanup_body(body: FunctionBody<'_>, name: &str, method: &str) -> bool {
    match body {
        FunctionBody::Arrow(body) => releases(body.expr(), name, method),
        FunctionBody::Block(block) => {
            let shadows_resource = block.stmts().any(|statement| {
                matches!(statement, Stmt::LocalVar(declaration)
                    if declaration.declarators().any(|variable| variable.name().name() == name))
            });
            !shadows_resource && block.stmts().any(|statement| {
                matches!(statement, Stmt::Expr(statement) if releases(statement.expr(), name, method))
            })
        }
        FunctionBody::Native(_) => false,
    }
}

fn borrowed_factory(
    program: Program<'_>,
    identity: &falcon_analyze::DeclarationIdentity,
    name: &str,
    state: &SemanticState<'_>,
) -> bool {
    program.declarations().any(|declaration| {
        let TopLevelDecl::Class(class) = declaration else { return false };
        if state.model.resolve_name(class.name().name()).as_ref() != Some(identity) {
            return false;
        }
        class.members().any(|member| {
            let ClassMember::Constructor(constructor) = member else { return false };
            if !constructor.is_factory() || constructor.constructor_name().map_or("new", |name| name.name()) != name {
                return false;
            }
            let borrowed = |expression: Expr<'_>| matches!(expression.unparenthesized(), Expr::Ident(_) | Expr::Field(_));
            match constructor.body() {
                Some(FunctionBody::Arrow(body)) => borrowed(body.expr()),
                Some(FunctionBody::Block(block)) => {
                    let mut statements = block.stmts();
                    matches!((statements.next(), statements.next()), (Some(Stmt::Return(statement)), None)
                        if statement.value().is_some_and(borrowed))
                }
                _ => false,
            }
        })
    })
}

fn release_target(field: FieldExpr<'_>, name: &str, method: &str) -> bool {
    field.field().name() == method
        && matches!(field.object().unparenthesized(), Expr::Ident(identifier) if identifier.name() == name)
}

fn releases(expression: Expr<'_>, name: &str, method: &str) -> bool {
    let mut invocation = ReleaseInvocation {
        name,
        method,
        found: false,
    };
    invocation.visit_expr(expression);
    invocation.found
}

struct ReleaseInvocation<'a> {
    name: &'a str,
    method: &'a str,
    found: bool,
}

impl<'a> Visitor<'a> for ReleaseInvocation<'_> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        match expression.unparenthesized() {
            Expr::Call(call) => {
                self.found = matches!(call.callee(), Expr::Field(field) if release_target(field, self.name, self.method))
                    || call.args().syntax().descendants().any(|node| {
                        matches!(Expr::cast(node), Some(Expr::Ident(identifier)) if identifier.name() == self.name)
                    });
            }
            Expr::Await(awaited) => walk_expr(self, Expr::Await(awaited)),
            _ => {}
        }
    }
}
