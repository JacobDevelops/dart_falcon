//! Avoid `await` on a returned `Future` the function's own `Future` already covers.
//!
//! Returning a `Future` from an `async` function already completes the
//! function's result with it, so awaiting first adds nothing:
//!
//! ```dart
//! Future<int> load() async => await fetch(); // BAD
//! Future<int> load() async => fetch(); // GOOD
//! ```
//!
//! Reported for `return await e;` statements and `=> await e` bodies (including
//! grouped expressions) of functions, methods, and closures returning `Future<R>`,
//! when `e` is a `Future<A>` with `A` provably assignable to `R`. Closures and
//! local functions infer their return type, flattening nested futures and
//! respecting a closure's contextual `Future` or `FutureOr` return type. Untyped top-level
//! declarations do not infer it.
//! Returns inside a `try` or `finally` block are exempt.
//!
//! Adopted from the official `unnecessary_await_in_return` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{BodyOwner, FunctionBodies};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UnnecessaryAwaitInReturn;

impl Rule for UnnecessaryAwaitInReturn {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diagnostics: Vec::new(),
            ctx,
            bodies: ctx.function_bodies(program),
            closure_returns: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    bodies: &'a FunctionBodies<'a>,
    closure_returns: Vec<(Span, Option<ResolvedType>)>,
}

impl<'a> Collector<'a> {
    fn check(&mut self, awaited: Expr<'a>, await_span: Span, state: &SemanticState<'a>) {
        let Some(body) = self.bodies.enclosing(&await_span) else {
            return;
        };
        if body.owner == BodyOwner::Constructor || self.bodies.inside_try_block(&await_span) {
            return;
        }
        let operand = state.infer(awaited);
        if !is_future(&operand) || operand.nullable() {
            return;
        }
        let unnecessary = match body.return_type {
            Some(return_type) => {
                let declared = state
                    .model
                    .resolve_type_in(return_type, &state.type_parameters);
                is_future(&declared)
                    && assignable(
                        &future_argument(&operand),
                        &future_argument(&declared),
                        state,
                    )
            }
            None if body.async_marker == AsyncMarker::Async
                && matches!(body.owner, BodyOwner::Closure | BodyOwner::LocalFunction) =>
            {
                let argument = future_argument(&operand);
                let inferred = flattened(argument.clone());
                let context = self
                    .closure_returns
                    .iter()
                    .find(|(span, _)| *span == body.span)
                    .and_then(|(_, ty)| ty.as_ref());
                match context {
                    Some(ResolvedType::Unknown) => false,
                    Some(context)
                        if is_future(context) || context.interface("dart:async", "FutureOr") =>
                    {
                        let contextual = future_argument(context);
                        let result = if assignable(&inferred, &contextual, state) {
                            &inferred
                        } else {
                            &contextual
                        };
                        assignable(&argument, result, state)
                    }
                    Some(ResolvedType::Void | ResolvedType::Dynamic) | None => {
                        assignable(&argument, &inferred, state)
                    }
                    Some(_) => false,
                }
            }
            None => false,
        };
        if unnecessary {
            self.diagnostics.push(self.ctx.diagnostic("Remove the unnecessary `await`.", Span::new(await_span.start, await_span.start + "await".len())));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::Return(statement) = node
            && let Some(Expr::Await(expression)) = statement.value().map(Expr::unparenthesized)
        {
            self.check(expression.expr(), expression.span(), state);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::FuncExpr(function) = node {
            let return_type = state.expected().map(|context| match context {
                ResolvedType::Function { .. } => context
                    .function_return_type()
                    .unwrap_or(ResolvedType::Unknown),
                ResolvedType::Dynamic | ResolvedType::Void => context.clone(),
                _ => ResolvedType::Unknown,
            });
            self.closure_returns
                .push((function.body().span(), return_type));
        }
        if let Expr::Await(expression) = node
            && self
                .bodies
                .enclosing(&node.span())
                .is_some_and(|body| body.arrow_expression == Some(node.span()))
        {
            self.check(expression.expr(), expression.span(), state);
        }
    }
}

fn is_future(ty: &ResolvedType) -> bool {
    ty.interface("dart:async", "Future")
}

/// `Future`'s type argument; a raw `Future` is `Future<dynamic>`.
fn future_argument(ty: &ResolvedType) -> ResolvedType {
    ty.type_argument(0).unwrap_or(ResolvedType::Dynamic)
}

fn flattened(mut ty: ResolvedType) -> ResolvedType {
    while is_future(&ty) || ty.interface("dart:async", "FutureOr") {
        ty = future_argument(&ty);
    }
    ty
}

/// Whether `actual` is provably a subtype of `expected`.
fn assignable(actual: &ResolvedType, expected: &ResolvedType, state: &SemanticState<'_>) -> bool {
    if matches!(actual, ResolvedType::Unknown) {
        return false;
    }
    if actual == expected || matches!(expected, ResolvedType::Dynamic | ResolvedType::Void) {
        return true;
    }
    if matches!(actual, ResolvedType::Never) {
        return true;
    }
    // `T & Object` (an asserted `T?`) and `T` are subtypes of `T` and `T?`,
    // and of whatever their bound is a subtype of.
    let parameter = match actual {
        ResolvedType::TypeParameter { id, nullable, .. } => Some((id, *nullable)),
        ResolvedType::NonNullTypeParameter { id, .. } => Some((id, false)),
        _ => None,
    };
    if let (
        Some((id, actual_nullable)),
        ResolvedType::TypeParameter {
            id: expected_id,
            nullable: expected_nullable,
            ..
        },
    ) = (parameter, expected)
        && id == expected_id
        && (!actual_nullable || *expected_nullable)
    {
        return true;
    }
    match actual {
        ResolvedType::TypeParameter {
            bound, nullable, ..
        } => {
            return assignable(
                &bound.with_nullable(*nullable || bound.nullable()),
                expected,
                state,
            );
        }
        ResolvedType::NonNullTypeParameter { bound, .. } => {
            return assignable(&bound.non_null(), expected, state);
        }
        _ => {}
    }
    if let (
        ResolvedType::Interface {
            identity: actual_identity,
            arguments: actual_arguments,
            nullable: actual_nullable,
            ..
        },
        ResolvedType::Interface {
            identity: expected_identity,
            arguments: expected_arguments,
            nullable: expected_nullable,
            ..
        },
    ) = (actual, expected)
        && actual_identity == expected_identity
        && (!*actual_nullable || *expected_nullable)
        && actual_arguments.len() == expected_arguments.len()
    {
        return actual_arguments
            .iter()
            .zip(expected_arguments.iter())
            .all(|(actual, expected)| assignable(actual, expected, state));
    }
    let ResolvedType::Interface {
        identity,
        arguments,
        ..
    } = expected
    else {
        return false;
    };
    if *identity == falcon_analyze::DeclarationIdentity::OBJECT {
        return expected.nullable()
            || matches!(
                actual,
                ResolvedType::Interface {
                    nullable: false,
                    ..
                }
            );
    }
    // SDK supertype instantiation preserves generic arguments (for example,
    // Socket's Stream<Uint8List>) rather than checking only its identity.
    match identity {
        falcon_analyze::DeclarationIdentity::Sdk { library, name } => {
            if !expected.nullable() && actual.nullable() {
                return false;
            }
            if arguments.is_empty() {
                state.signatures.implements_sdk(actual, library, name) == Truth::Yes
            } else {
                state
                    .signatures
                    .instantiated_supertype(actual, library, name, &state.model)
                    .is_some_and(|supertype| {
                        supertype != *actual && assignable(&supertype, expected, state)
                    })
            }
        }
        _ if arguments.is_empty() => {
            (expected.nullable() || !actual.nullable())
                && state
                    .signatures
                    .is_subtype_of(actual, identity, &state.model)
                    == Truth::Yes
        }
        _ => false,
    }
}
