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
//! Reported for `return await e;` statements and `=> await e` bodies of
//! functions and methods whose declared return type is `Future<R>`, when `e`
//! is a `Future<A>` with `A` provably assignable to `R`. Returns inside a
//! `try` or `finally` block are exempt (awaiting there changes which errors the
//! handlers see), and closures are skipped because their return type is
//! inferred.
//!
//! Adopted from the official `unnecessary_await_in_return` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::function_bodies::{BodyOwner, FunctionBodies};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnnecessaryAwaitInReturn;

impl Rule for UnnecessaryAwaitInReturn {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            bodies: FunctionBodies::new(program),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies,
}

impl Collector<'_> {
    fn check(&mut self, awaited: &Expr, await_span: Span, state: &SemanticState<'_>) {
        let Some(body) = self.bodies.enclosing(&await_span) else {
            return;
        };
        if matches!(body.owner, BodyOwner::Closure | BodyOwner::Constructor)
            || self.bodies.inside_try_block(&await_span)
        {
            return;
        }
        let Some(return_type) = &body.return_type else {
            return;
        };
        let declared = state
            .model
            .resolve_type_in(return_type, &state.type_parameters);
        let operand = state.infer(awaited);
        if !is_future(&declared) || !is_future(&operand) || operand.nullable() {
            return;
        }
        if assignable(
            &future_argument(&operand),
            &future_argument(&declared),
            state,
        ) {
            self.diagnostics.push(self.ctx.diagnostic(
                "unnecessary-await-in-return",
                "Unnecessary 'await'.",
                Span::new(await_span.start, await_span.start + "await".len()),
            ));
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        if let Stmt::Return(statement) = node
            && let Some(Expr::Await { expr, span }) = &statement.value
        {
            self.check(expr, *span, state);
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Await { expr, span } = node
            && self
                .bodies
                .enclosing(span)
                .is_some_and(|body| body.arrow_expression == Some(*span))
        {
            self.check(expr, *span, state);
        }
    }
}

fn is_future(ty: &ResolvedType) -> bool {
    ty.interface("dart:async", "Future")
}

/// `Future`'s type argument; a raw `Future` is `Future<dynamic>`.
fn future_argument(ty: &ResolvedType) -> ResolvedType {
    ty.arguments()
        .first()
        .cloned()
        .unwrap_or(ResolvedType::Dynamic)
}

/// Whether `actual` is provably a subtype of `expected`.
fn assignable(actual: &ResolvedType, expected: &ResolvedType, state: &SemanticState<'_>) -> bool {
    if matches!(actual, ResolvedType::Unknown) {
        return false;
    }
    if actual == expected || matches!(expected, ResolvedType::Dynamic | ResolvedType::Void) {
        return true;
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
    // Non-generic SDK supertypes reached through a resolved declaration chain.
    match identity {
        falcon_analyze::DeclarationIdentity::Sdk { library, name } if arguments.is_empty() => {
            (expected.nullable() || !actual.nullable())
                && matches!(actual, ResolvedType::Interface { .. })
                && state.signatures.implements_sdk(actual, library, name) == Truth::Yes
        }
        _ => false,
    }
}
