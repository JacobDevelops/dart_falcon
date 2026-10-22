//! Shared unused-Future detection and annotation exemptions.

use falcon_analyze::FunctionBodies;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, ResolvedTypeView, Truth,
};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::HashSet;
/// The shared shape of `discarded-futures` and `unawaited-futures`: both report
/// unused future-typed expressions and differ only in which bodies they watch
/// and whether `FutureOr` counts.
pub(crate) struct UnusedFuturesRule {
    /// `true` watches `async`/`async*` bodies, `false` synchronous ones.
    pub(crate) in_async_bodies: bool,
    pub(crate) future_message: &'static str,
    /// The message for `FutureOr` results; `None` leaves them unreported.
    pub(crate) future_or_message: Option<&'static str>,
}

pub(crate) fn unused_futures<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    rule: &'a UnusedFuturesRule,
) -> Box<dyn SemanticRuleVisitor<'a> + 'a> {
    let collector = Collector {
        diagnostics: Vec::new(),
        ctx,
        rule,
        bodies: ctx.function_bodies(program),
        annotated_local_functions: HashSet::new(),
    };
    Box::new(collector)
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    rule: &'a UnusedFuturesRule,
    bodies: &'a FunctionBodies<'a>,
    annotated_local_functions: HashSet<Span>,
}

impl<'a> Collector<'a> {
    fn interesting(&self, span: &Span) -> bool {
        self.bodies
            .enclosing(span)
            .is_some_and(|body| body.async_marker.is_async() == self.rule.in_async_bodies)
    }

    /// The message for an unused value of type `ty`, if this rule reports it.
    /// Like the analyzer's `asInstanceOf`, nullability doesn't matter.
    fn message(&self, ty: &ResolvedType, state: &SemanticState<'_>) -> Option<&'static str> {
        if ty.interface("dart:async", "FutureOr") {
            return self.rule.future_or_message;
        }
        (matches!(ty.view(), ResolvedTypeView::Interface { .. })
            && state.signatures.implements_sdk(ty, "dart:async", "Future") == Truth::Yes)
            .then_some(self.rule.future_message)
    }

    fn report(&mut self, message: &'static str, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(message, span));
    }

    fn local_await_not_required(&self, expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let Expr::Call(call) = expr.unparenthesized() else {
            return false;
        };
        // `.call` resolves to the function type's member, not this declaration.
        let Expr::Ident(name) = call.callee().unparenthesized() else {
            return false;
        };
        state
            .environment
            .binding_declaration(name.name())
            .is_some_and(|declaration| self.annotated_local_functions.contains(&declaration))
    }

    fn check_expression(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(expr.unparenthesized(), Expr::Assign(_) | Expr::Await(_))
            || !self.interesting(&expr.span())
            || invokes_await_not_required(expr, state)
            || self.local_await_not_required(expr, state)
            || future_delayed_with_computation(expr, state)
            || map_put_if_absent(expr, state)
        {
            return;
        }
        if let Some(message) = self.message(&state.infer(expr), state) {
            self.report(message, report_span(expr, state, self.ctx));
        }
    }

    fn check_interpolation(&mut self, expr: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(expr.unparenthesized(), Expr::Assign(_))
            || !self.interesting(&expr.span())
            || invokes_await_not_required(expr, state)
            || self.local_await_not_required(expr, state)
        {
            return;
        }
        if let Some(message) = self.message(&state.infer(expr), state) {
            self.report(message, report_span(expr, state, self.ctx));
        }
    }

    fn check_cascade(
        &mut self,
        object: Expr<'a>,
        sections: AstChildren<'a, CascadeSection<'a>>,
        is_null_aware: bool,
        state: &SemanticState<'a>,
    ) {
        if !self.interesting(&object.span()) {
            return;
        }
        let target = state.infer_receiver(object);
        let target = if is_null_aware {
            target.non_null()
        } else {
            target
        };
        for section in sections {
            let mut receiver = target.clone();
            let mut reported = None;
            let mut callee_end = section.span().start;
            for op in section.ops() {
                let previous = std::mem::replace(&mut receiver, ResolvedType::UNKNOWN);
                (receiver, reported) = match op {
                    CascadeOp::NullAssert(_) => (previous.non_null(), reported),
                    CascadeOp::Call(call) => {
                        let name = call.name();
                        let callable = callable_member(&previous, name.name(), state);
                        let result = if callable {
                            state.cascade_invoke_return_type(
                                &field_type(&previous, name.name(), state),
                                call.type_args(),
                                call.args(),
                            )
                        } else {
                            member_return_type(&previous, name.name(), state)
                        };
                        (
                            result,
                            Some((
                                member_await_not_required(&previous, name.name(), state),
                                if callable {
                                    Span::new(section.span().start, name.span().end)
                                } else {
                                    name.span()
                                },
                            )),
                        )
                    }
                    CascadeOp::Field(field) => (
                        field_type(&previous, field.name().name(), state),
                        Some((
                            member_await_not_required(&previous, field.name().name(), state),
                            field.name().span(),
                        )),
                    ),
                    CascadeOp::Invoke(invoke) => (
                        state.cascade_invoke_return_type(
                            &previous,
                            invoke.type_args(),
                            invoke.args(),
                        ),
                        Some((
                            type_alias_await_not_required(&previous, state),
                            Span::new(section.span().start, callee_end),
                        )),
                    ),
                    CascadeOp::Index(_) => (member_return_type(&previous, "[]", state), None),
                    CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) => {
                        (ResolvedType::UNKNOWN, None)
                    }
                };
                callee_end = match op {
                    CascadeOp::Field(field) => field.name().span().end,
                    CascadeOp::Call(call) => call.args().span().end,
                    CascadeOp::Invoke(invoke) => invoke.args().span().end,
                    CascadeOp::Index(index) => index.span().end,
                    CascadeOp::NullAssert(assertion) => assertion.span().end,
                    CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) => section.span().end,
                };
            }
            if let Some((await_not_required, span)) = reported
                && !await_not_required
                && let Some(message) = self.message(&receiver, state)
            {
                self.report(message, span);
            }
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            Stmt::Expr(statement) => self.check_expression(statement.expr(), state),
            Stmt::LocalFunc(function)
                if function.annotations().any(|annotation| {
                    let mut segments = annotation.name();
                    let name = match (segments.next(), segments.next(), segments.next()) {
                        (Some(name), None, None) => QualifiedName::from(name.name()),
                        (Some(prefix), Some(name), None) => {
                            QualifiedName::prefixed(prefix.name(), name.name())
                        }
                        _ => return false,
                    };
                    state
                        .model
                        .resolve_imported_member(name)
                        .is_some_and(|identity| is_await_not_required(&identity))
                }) =>
            {
                self.annotated_local_functions
                    .insert(function.name().span());
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Cascade(cascade) => self.check_cascade(
                cascade.object(),
                cascade.sections(),
                cascade.is_null_aware(),
                state,
            ),
            Expr::StringLit(literal) => {
                for interpolation in literal.interpolations() {
                    self.check_interpolation(interpolation.expr(), state);
                }
            }
            _ => {}
        }
    }
}

fn member_return_type(
    receiver: &ResolvedType,
    name: &str,
    state: &SemanticState<'_>,
) -> ResolvedType {
    state
        .signatures
        .resolved_member(receiver, name, &state.model)
        .map(|(signature, substitutions)| {
            state
                .model
                .substitute(&signature.return_type, &substitutions)
        })
        .unwrap_or(ResolvedType::UNKNOWN)
}

fn field_type(receiver: &ResolvedType, name: &str, state: &SemanticState<'_>) -> ResolvedType {
    state
        .signatures
        .resolved_field(receiver, name, &state.model)
        .or_else(|| {
            state
                .signatures
                .resolved_extension_getter(receiver, name, &state.model)
        })
        .map(|(ty, substitutions)| state.model.substitute(&ty, &substitutions))
        .unwrap_or(ResolvedType::UNKNOWN)
}

fn callable_member(receiver: &ResolvedType, name: &str, state: &SemanticState<'_>) -> bool {
    state.is_callable_value_member(receiver, name)
        || state
            .signatures
            .resolved_extension_getter(receiver, name, &state.model)
            .is_some()
}

fn is_await_not_required(annotation: &DeclarationIdentity) -> bool {
    annotation.is_package("meta", "awaitNotRequired")
}

/// Whether instance member `name` of `receiver`, or its declaration in a
/// supertype, is annotated `@awaitNotRequired`.
fn member_await_not_required(
    receiver: &ResolvedType,
    name: &str,
    state: &SemanticState<'_>,
) -> bool {
    state
        .signatures
        .resolved_member_annotated(receiver, name, &state.model, is_await_not_required)
}

fn type_alias_await_not_required(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    ty.function_alias_identity().is_some_and(|identity| {
        state
            .signatures
            .declaration(identity)
            .is_some_and(|facts| facts.annotations.iter().any(is_await_not_required))
    })
}

/// The analyzer's `isAwaitNotRequired`: the invoked top-level function,
/// static or instance method, or getter (or a supertype declaration it overrides) is annotated
/// `@awaitNotRequired`.
fn invokes_await_not_required<'a>(expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
    if let Expr::Call(call) = expr.unparenthesized() {
        let callee = call.callee();
        let explicit_call = matches!(callee.unparenthesized(), Expr::Field(field)
            if field.field().name() == "call"
                && matches!(state.infer_receiver(field.object()).view(), ResolvedTypeView::Function { .. }));
        if !explicit_call && type_alias_await_not_required(&state.infer_receiver(callee), state) {
            return true;
        }
    }
    let function = |name: QualifiedName<'_>| {
        state.model.resolve_value(name).is_some_and(|identity| {
            state
                .signatures
                .function_annotations(&identity)
                .iter()
                .any(is_await_not_required)
        })
    };
    let static_member = |owner: &DeclarationIdentity, name: &str| {
        state
            .signatures
            .declared_member_facts(owner, name)
            .iter()
            .any(|fact| fact.is_static && fact.annotations.iter().any(is_await_not_required))
    };
    let member = |object: Expr<'a>, name: &str, is_null_safe: bool| {
        if let Expr::Ident(prefix) = object.unparenthesized()
            && !state.environment.is_bound(prefix.name())
            && state
                .model
                .resolve_value(QualifiedName::prefixed(prefix.name(), name))
                .is_some()
        {
            return function(QualifiedName::prefixed(prefix.name(), name));
        }
        if let Expr::Ident(class) = object.unparenthesized()
            && !state.environment.is_bound(class.name())
            && let Some(identity) = state.model.resolve_name(class.name())
        {
            return static_member(&identity, name);
        }
        if let Expr::Call(call) = object.unparenthesized()
            && let Some(identity) = state.named_extension(call.callee())
            && state
                .signatures
                .extension_member_annotated(&identity, name, is_await_not_required)
        {
            return true;
        }
        member_await_not_required(
            &state.infer_access_receiver(object, name, is_null_safe),
            name,
            state,
        )
    };
    match expr.unparenthesized() {
        Expr::Call(call) => match call.callee().unparenthesized() {
            Expr::Ident(identifier) if !state.environment.is_bound(identifier.name()) => {
                state.enclosing_type.as_ref().is_some_and(|receiver| {
                    member_await_not_required(receiver, identifier.name(), state)
                        || matches!(receiver.view(), ResolvedTypeView::Interface { identity, .. }
                            if static_member(identity, identifier.name()))
                }) || function(QualifiedName::from(identifier.name()))
            }
            Expr::Field(field) => {
                member(field.object(), field.field().name(), field.is_null_safe())
            }
            _ => false,
        },
        Expr::Field(field) => member(field.object(), field.field().name(), field.is_null_safe()),
        Expr::Binary(binary) => binary.op().operator_name().is_some_and(|name| {
            member_await_not_required(&state.infer_receiver(binary.left()), name, state)
        }),
        Expr::Unary(unary) => match unary.op() {
            UnaryOp::Minus => {
                member_await_not_required(&state.infer_receiver(unary.operand()), "unary-", state)
            }
            UnaryOp::Tilde => {
                member_await_not_required(&state.infer_receiver(unary.operand()), "~", state)
            }
            _ => false,
        },
        _ => false,
    }
}

/// Only the SDK `Future.delayed` constructor's computation form is exempt.
fn future_delayed_with_computation<'a>(expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let (is_delayed, args) = match expr.unparenthesized() {
        Expr::Call(call) => {
            let callee = call.callee();
            let constructor = falcon_analyze::expression_segments(callee, true)
                .filter(|segments| {
                    segments
                        .first()
                        .is_some_and(|name| !state.environment.is_bound(name))
                })
                .and_then(|_| falcon_analyze::resolve_call_constructor(callee, &state.model, true));
            (
                constructor.is_some_and(|(identity, name)| {
                    identity.is_sdk("dart:async", "Future") && name == "delayed"
                }),
                call.args(),
            )
        }
        Expr::New(new) if new.constructor_name().is_some() => (
            new.constructor_name().unwrap().name() == "delayed"
                && state
                    .model
                    .resolve_type(new.dart_type())
                    .interface("dart:async", "Future"),
            new.args(),
        ),
        _ => return false,
    };
    is_delayed && args.positional().count() + args.named().count() == 2
}

/// `map.putIfAbsent(...)` on a `dart:core` `Map`: the official lints exempt it.
fn map_put_if_absent<'a>(expr: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let Expr::Call(call) = expr.unparenthesized() else {
        return false;
    };
    let Expr::Field(field) = call.callee().unparenthesized() else {
        return false;
    };
    field.field().name() == "putIfAbsent"
        && state
            .signatures
            .resolved_member_owner(
                &state.infer_receiver(field.object()),
                field.field().name(),
                &state.model,
            )
            .is_some_and(|owner| owner.is_sdk("dart:core", "Map"))
}

/// Method invocations report the method name. Function-expression invocations
/// report their complete callee; constructors report the complete constructor name.
fn report_span<'a>(expr: Expr<'a>, state: &SemanticState<'a>, ctx: &AnalyzeContext<'_>) -> Span {
    match expr {
        Expr::Call(call) => {
            let callee = call.callee();
            if state.is_constructor_invocation(callee) {
                let end = ctx
                    .tree()
                    .tokens_in(
                        ctx.source(),
                        Span::new(callee.span().start, call.args().span().start),
                    )
                    .rfind(|token| !token.is_trivia())
                    .map_or(callee.span().end, |token| token.offset() + token.len());
                return Span::new(callee.span().start, end);
            }
            if let Expr::Field(field) = callee
                && !state.is_function_expression_invocation(callee)
                && !callable_member(
                    &state.infer_access_receiver(
                        field.object(),
                        field.field().name(),
                        field.is_null_safe(),
                    ),
                    field.field().name(),
                    state,
                )
            {
                field.field().span()
            } else {
                callee.span()
            }
        }
        Expr::New(new) => Span::new(
            new.dart_type().span().start,
            new.constructor_name()
                .map_or(new.dart_type().span().end, |name| name.span().end),
        ),
        Expr::Field(field) => field.field().span(),
        other => other.span(),
    }
}
