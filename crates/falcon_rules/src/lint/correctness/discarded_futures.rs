//! Don't discard `Future`s returned by calls in synchronous functions.
//!
//! A `Future` created inside a non-`async` function and then dropped can't be
//! awaited, so its errors and completion are lost:
//!
//! ```dart
//! void save() {
//!   writeFile(); // BAD
//! }
//!
//! Future<void> save() async {
//!   await writeFile(); // GOOD
//! }
//! ```
//!
//! Expression statements, cascade sections, and string interpolations whose
//! resolved type is `Future` (or a subtype) or `FutureOr` are reported inside
//! synchronous bodies. Assignments, `Future.delayed(duration, computation)`,
//! `Map.putIfAbsent`, and members annotated `@awaitNotRequired`, including
//! inherited, extension, and local functions, are exempt; local annotations
//! follow the visible declaration rather than a reusable name. Function typedef
//! annotations follow the invoked value's immediate alias through inferred
//! variables, fields, records, and collections, but not explicit `.call` or an
//! unannotated alias of that typedef. Member calls, including null-aware
//! accesses, use the reached receiver branch for annotation lookup, as do
//! bounded type parameters and flow-promoted `FutureOr` values. Unresolved
//! types stay silent.
//!
//! Adopted from the official `discarded_futures` lint.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, Rule, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::function_bodies::FunctionBodies;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct DiscardedFutures;

impl Rule for DiscardedFutures {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        unused_futures(
            program,
            ctx,
            &UnusedFuturesRule {
                name: "discarded-futures",
                in_async_bodies: false,
                future_message: "Asynchronous function invoked in a non-'async' function.",
                future_or_message: Some(
                    "Function returning 'FutureOr' invoked in a non-'async' function.",
                ),
            },
        )
    }
}

/// The shared shape of `discarded-futures` and `unawaited-futures`: both report
/// unused future-typed expressions and differ only in which bodies they watch
/// and whether `FutureOr` counts.
pub(crate) struct UnusedFuturesRule {
    pub(crate) name: &'static str,
    /// `true` watches `async`/`async*` bodies, `false` synchronous ones.
    pub(crate) in_async_bodies: bool,
    pub(crate) future_message: &'static str,
    /// The message for `FutureOr` results; `None` leaves them unreported.
    pub(crate) future_or_message: Option<&'static str>,
}

pub(crate) fn unused_futures(
    program: &Program,
    ctx: &AnalyzeContext,
    rule: &UnusedFuturesRule,
) -> Vec<Diagnostic> {
    let Some(state) = SemanticState::new(program, ctx) else {
        return Vec::new();
    };
    let mut collector = Collector {
        diagnostics: Vec::new(),
        ctx,
        rule,
        bodies: FunctionBodies::new(program),
        annotated_local_functions: HashSet::new(),
    };
    visit_program(&mut collector, program, state);
    collector.diagnostics
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    rule: &'a UnusedFuturesRule,
    bodies: FunctionBodies,
    annotated_local_functions: HashSet<Span>,
}

impl Collector<'_> {
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
        (matches!(ty, ResolvedType::Interface { .. })
            && state.signatures.implements_sdk(ty, "dart:async", "Future") == Truth::Yes)
            .then_some(self.rule.future_message)
    }

    fn report(&mut self, message: &'static str, span: Span) {
        self.diagnostics
            .push(self.ctx.diagnostic(self.rule.name, message, span));
    }

    fn local_await_not_required(&self, expr: &Expr, state: &SemanticState<'_>) -> bool {
        let Expr::Call { callee, .. } = expr.unparenthesized() else {
            return false;
        };
        // `.call` resolves to the function type's member, not this declaration.
        let Expr::Ident(name) = callee.unparenthesized() else {
            return false;
        };
        state
            .environment
            .binding_declaration(&name.name)
            .is_some_and(|declaration| self.annotated_local_functions.contains(&declaration))
    }

    fn check_expression(&mut self, expr: &Expr, state: &SemanticState<'_>) {
        if matches!(
            expr.unparenthesized(),
            Expr::Assign { .. } | Expr::Await { .. }
        ) || !self.interesting(expr.span())
            || invokes_await_not_required(expr, state)
            || self.local_await_not_required(expr, state)
            || future_delayed_with_computation(expr, state)
            || map_put_if_absent(expr, state)
        {
            return;
        }
        if let Some(message) = self.message(&state.infer(expr), state) {
            self.report(message, report_span(expr, state));
        }
    }

    fn check_interpolation(&mut self, expr: &Expr, state: &SemanticState<'_>) {
        if matches!(expr.unparenthesized(), Expr::Assign { .. })
            || !self.interesting(expr.span())
            || invokes_await_not_required(expr, state)
            || self.local_await_not_required(expr, state)
        {
            return;
        }
        if let Some(message) = self.message(&state.infer(expr), state) {
            self.report(message, report_span(expr, state));
        }
    }

    fn check_cascade(
        &mut self,
        object: &Expr,
        sections: &[CascadeSection],
        state: &SemanticState<'_>,
    ) {
        if !self.interesting(object.span()) {
            return;
        }
        let target = state.infer_receiver(object);
        for section in sections {
            let mut receiver = target.clone();
            let mut reported = None;
            let mut callee_end = section.span.start;
            for op in &section.ops {
                let previous = std::mem::replace(&mut receiver, ResolvedType::Unknown);
                (receiver, reported) = match op {
                    CascadeOp::NullAssert { .. } => (previous.non_null(), reported),
                    CascadeOp::Call(name, _, _) => (
                        member_return_type(&previous, &name.name, state),
                        Some((
                            member_await_not_required(&previous, &name.name, state),
                            name.span,
                        )),
                    ),
                    CascadeOp::Field { name, .. } => (
                        field_type(&previous, &name.name, state),
                        Some((
                            member_await_not_required(&previous, &name.name, state),
                            name.span,
                        )),
                    ),
                    CascadeOp::Invoke(type_args, args) => (
                        state.cascade_invoke_return_type(&previous, type_args, args),
                        Some((
                            type_alias_await_not_required(&previous, state),
                            Span::new(section.span.start, callee_end),
                        )),
                    ),
                    CascadeOp::Index { .. } => (member_return_type(&previous, "[]", state), None),
                    CascadeOp::Assign(..) | CascadeOp::IndexAssign(..) => {
                        (ResolvedType::Unknown, None)
                    }
                };
                callee_end = match op {
                    CascadeOp::Field { name, .. } => name.span.end,
                    CascadeOp::Call(_, _, args) | CascadeOp::Invoke(_, args) => args.span.end,
                    CascadeOp::Index { span, .. } | CascadeOp::NullAssert { span } => span.end,
                    CascadeOp::Assign(..) | CascadeOp::IndexAssign(..) => section.span.end,
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

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        match node {
            Stmt::Expr(statement) => self.check_expression(&statement.expr, state),
            Stmt::LocalFunc(function)
                if function.annotations.iter().any(|annotation| {
                    let name = match annotation.name.as_slice() {
                        [name] => QualifiedName::from(&name.name),
                        [prefix, name] => QualifiedName::prefixed(&prefix.name, &name.name),
                        _ => return false,
                    };
                    state
                        .model
                        .resolve_imported_member(name)
                        .is_some_and(|identity| is_await_not_required(&identity))
                }) =>
            {
                self.annotated_local_functions.insert(function.name.span);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Cascade {
                object, sections, ..
            } => self.check_cascade(object, sections, state),
            Expr::StringLit(literal) => {
                for interpolation in &literal.interpolations {
                    self.check_interpolation(&interpolation.expr, state);
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
        .unwrap_or(ResolvedType::Unknown)
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
        .unwrap_or(ResolvedType::Unknown)
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
    matches!(ty, ResolvedType::Function { alias, .. }
    if alias.identity().is_some_and(|identity| {
        state.signatures.declaration(identity)
            .is_some_and(|facts| facts.annotations.iter().any(is_await_not_required))
    }))
}

/// The analyzer's `isAwaitNotRequired`: the invoked top-level function,
/// static or instance method, or getter (or a supertype declaration it overrides) is annotated
/// `@awaitNotRequired`.
fn invokes_await_not_required(expr: &Expr, state: &SemanticState<'_>) -> bool {
    if let Expr::Call { callee, .. } = expr.unparenthesized() {
        let explicit_call = matches!(callee.unparenthesized(), Expr::Field { object, field, .. }
            if field.name == "call"
                && matches!(state.infer_receiver(object), ResolvedType::Function { .. }));
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
    let member = |object: &Expr, name: &str, is_null_safe: bool| {
        if let Expr::Ident(prefix) = object.unparenthesized()
            && !state.environment.is_bound(&prefix.name)
            && state
                .model
                .resolve_value(QualifiedName::prefixed(&prefix.name, name))
                .is_some()
        {
            return function(QualifiedName::prefixed(&prefix.name, name));
        }
        if let Expr::Ident(class) = object.unparenthesized()
            && !state.environment.is_bound(&class.name)
            && let Some(identity) = state.model.resolve_name(&class.name)
        {
            return static_member(&identity, name);
        }
        if let Expr::Call { callee, .. } = object.unparenthesized()
            && let Some(identity) = state.named_extension(callee)
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
        Expr::Call { callee, .. } => match callee.unparenthesized() {
            Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                state.enclosing_type.as_ref().is_some_and(|receiver| {
                    member_await_not_required(receiver, &identifier.name, state)
                        || matches!(receiver, ResolvedType::Interface { identity, .. }
                            if static_member(identity, &identifier.name))
                }) || function(QualifiedName::from(&identifier.name))
            }
            Expr::Field {
                object,
                field,
                is_null_safe,
                ..
            } => member(object, &field.name, *is_null_safe),
            _ => false,
        },
        Expr::Field {
            object,
            field,
            is_null_safe,
            ..
        } => member(object, &field.name, *is_null_safe),
        Expr::Binary { op, left, .. } => op.method_name().is_some_and(|name| {
            member_await_not_required(&state.infer_receiver(left), name, state)
        }),
        Expr::Unary { op, operand, .. } => match op {
            UnaryOp::Minus => {
                member_await_not_required(&state.infer_receiver(operand), "unary-", state)
            }
            UnaryOp::Tilde => member_await_not_required(&state.infer_receiver(operand), "~", state),
            _ => false,
        },
        _ => false,
    }
}

/// Only the SDK `Future.delayed` constructor's computation form is exempt.
fn future_delayed_with_computation(expr: &Expr, state: &SemanticState<'_>) -> bool {
    let (is_delayed, args) = match expr.unparenthesized() {
        Expr::Call { callee, args, .. } => {
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
                args,
            )
        }
        Expr::New {
            dart_type,
            constructor_name: Some(constructor),
            args,
            ..
        } => (
            constructor.name == "delayed"
                && state
                    .model
                    .resolve_type(dart_type)
                    .interface("dart:async", "Future"),
            args,
        ),
        _ => return false,
    };
    is_delayed && args.positional.len() + args.named.len() == 2
}

/// `map.putIfAbsent(...)` on a `dart:core` `Map`: the official lints exempt it.
fn map_put_if_absent(expr: &Expr, state: &SemanticState<'_>) -> bool {
    let Expr::Call { callee, .. } = expr.unparenthesized() else {
        return false;
    };
    let Expr::Field { object, field, .. } = callee.unparenthesized() else {
        return false;
    };
    field.name == "putIfAbsent"
        && state
            .signatures
            .resolved_member_owner(&state.infer_receiver(object), &field.name, &state.model)
            .is_some_and(|owner| owner.is_sdk("dart:core", "Map"))
}

/// Method invocations report the method name. Function-expression invocations
/// report their complete callee, including parentheses and qualified properties.
fn report_span(expr: &Expr, state: &SemanticState<'_>) -> Span {
    match expr {
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field {
                object,
                field,
                is_null_safe,
                ..
            } => {
                let named_property = match object.as_ref() {
                    Expr::Ident(name) if !state.environment.is_bound(&name.name) => {
                        state
                            .model
                            .resolve_value(QualifiedName::prefixed(&name.name, &field.name))
                            .is_some_and(|identity| {
                                state.signatures.value_type(&identity).is_some()
                            })
                            || state
                                .model
                                .resolve_name(&name.name)
                                .is_some_and(|identity| {
                                    state.signatures.field(&identity, &field.name).is_some()
                                })
                    }
                    Expr::Field {
                        object: prefix,
                        field: class,
                        is_null_safe: false,
                        ..
                    } => matches!(prefix.as_ref(), Expr::Ident(prefix)
                        if !state.environment.is_bound(&prefix.name)
                            && state.model.resolve_name(QualifiedName::prefixed(
                                &prefix.name, &class.name,
                            )).is_some_and(|identity|
                                state.signatures.field(&identity, &field.name).is_some())),
                    _ => false,
                };
                let function_property = named_property || {
                    let receiver = state.infer_access_receiver(object, &field.name, *is_null_safe);
                    receiver.record_field(&field.name).is_some()
                        || state
                            .signatures
                            .resolved_field(&receiver, &field.name, &state.model)
                            .is_some()
                        || state
                            .signatures
                            .resolved_extension_getter(&receiver, &field.name, &state.model)
                            .is_some()
                };
                if function_property {
                    *callee.span()
                } else {
                    field.span
                }
            }
            other => *other.span(),
        },
        Expr::Field { field, .. } => field.span,
        other => *other.span(),
    }
}
