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
//! `Map.putIfAbsent`, and members annotated `@awaitNotRequired` in this file
//! are exempt; nullable or unresolved types stay silent.
//!
//! Adopted from the official `discarded_futures` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_class_member, walk_function_decl, walk_program};

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
                future_or_message: "Function returning 'FutureOr' invoked in a non-'async' function.",
            },
        )
    }
}

/// The shared shape of `discarded-futures` and `unawaited-futures`: both report
/// unused future-typed expressions and differ only in which bodies they watch.
pub(crate) struct UnusedFuturesRule {
    pub(crate) name: &'static str,
    /// `true` watches `async`/`async*` bodies, `false` synchronous ones.
    pub(crate) in_async_bodies: bool,
    pub(crate) future_message: &'static str,
    pub(crate) future_or_message: &'static str,
}

pub(crate) fn unused_futures(
    program: &Program,
    ctx: &AnalyzeContext,
    rule: &UnusedFuturesRule,
) -> Vec<Diagnostic> {
    let Some(state) = SemanticState::new(program, ctx) else {
        return Vec::new();
    };
    let mut exempt = AwaitNotRequired::default();
    walk_program(&mut exempt, program);
    let mut collector = Collector {
        diagnostics: Vec::new(),
        ctx,
        rule,
        bodies: FunctionBodies::new(program),
        await_not_required: exempt.names,
    };
    visit_program(&mut collector, program, state);
    collector.diagnostics
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FutureKind {
    Future,
    FutureOr,
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    rule: &'a UnusedFuturesRule,
    bodies: FunctionBodies,
    await_not_required: HashSet<String>,
}

impl Collector<'_> {
    fn interesting(&self, span: &Span) -> bool {
        self.bodies
            .enclosing(span)
            .is_some_and(|body| body.async_marker.is_async() == self.rule.in_async_bodies)
    }

    fn report(&mut self, kind: FutureKind, span: Span) {
        let message = match kind {
            FutureKind::Future => self.rule.future_message,
            FutureKind::FutureOr => self.rule.future_or_message,
        };
        self.diagnostics
            .push(self.ctx.diagnostic(self.rule.name, message, span));
    }

    /// Whether the invoked member is annotated `@awaitNotRequired` in this file.
    fn await_not_required(&self, expr: &Expr) -> bool {
        let name = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(identifier) => Some(&identifier.name),
                Expr::Field { field, .. } => Some(&field.name),
                _ => None,
            },
            Expr::Field { field, .. } => Some(&field.name),
            Expr::Ident(identifier) => Some(&identifier.name),
            _ => None,
        };
        name.is_some_and(|name| self.await_not_required.contains(name))
    }

    fn check_expression(&mut self, expr: &Expr, state: &SemanticState<'_>) {
        if matches!(expr, Expr::Assign { .. } | Expr::Await { .. })
            || self.await_not_required(expr)
            || future_delayed_with_computation(expr)
            || map_put_if_absent(expr, state)
            || !self.interesting(expr.span())
        {
            return;
        }
        if let Some(kind) = future_kind(&state.infer(expr), state) {
            self.report(kind, report_span(expr));
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
        let target = state.infer(object);
        for section in sections {
            let mut receiver = target.clone();
            let mut reported = None;
            for op in &section.ops {
                (receiver, reported) = match op {
                    CascadeOp::Call(name, _, _) => {
                        (member_return_type(&receiver, &name.name, state), Some(name))
                    }
                    CascadeOp::Field { name, .. } => {
                        (field_type(&receiver, &name.name, state), Some(name))
                    }
                    CascadeOp::Index { .. } | CascadeOp::Assign(..) => {
                        (ResolvedType::Unknown, None)
                    }
                };
            }
            if let Some(name) = reported
                && !self.await_not_required.contains(&name.name)
                && let Some(kind) = future_kind(&receiver, state)
            {
                self.report(kind, name.span);
            }
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        if let Stmt::Expr(statement) = node {
            self.check_expression(&statement.expr, state);
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Cascade {
                object, sections, ..
            } => self.check_cascade(object, sections, state),
            Expr::StringLit(literal) => {
                for interpolation in &literal.interpolations {
                    self.check_expression(&interpolation.expr, state);
                }
            }
            _ => {}
        }
    }
}

fn future_kind(ty: &ResolvedType, state: &SemanticState<'_>) -> Option<FutureKind> {
    if ty.nullable() {
        return None;
    }
    if ty.interface("dart:async", "FutureOr") {
        return Some(FutureKind::FutureOr);
    }
    (state.signatures.implements_sdk(ty, "dart:async", "Future") == Truth::Yes)
        .then_some(FutureKind::Future)
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
        .map(|(ty, substitutions)| state.model.substitute(&ty, &substitutions))
        .unwrap_or(ResolvedType::Unknown)
}

/// `Future.delayed(duration, computation)`: the official lints exempt it.
fn future_delayed_with_computation(expr: &Expr) -> bool {
    let (is_delayed, args) = match expr {
        Expr::Call { callee, args, .. } => (
            matches!(callee.as_ref(), Expr::Field { object, field, .. }
                if field.name == "delayed"
                    && matches!(object.as_ref(), Expr::Ident(target) if target.name == "Future")),
            args,
        ),
        Expr::New {
            dart_type: DartType::Named(named),
            constructor_name: Some(constructor),
            args,
            ..
        } => (
            constructor.name == "delayed"
                && matches!(named.segments.last(), Some(name) if name.name == "Future"),
            args,
        ),
        _ => return false,
    };
    is_delayed && args.positional.len() + args.named.len() == 2
}

/// `map.putIfAbsent(...)` on a `dart:core` `Map`: the official lints exempt it.
fn map_put_if_absent(expr: &Expr, state: &SemanticState<'_>) -> bool {
    let Expr::Call { callee, .. } = expr else {
        return false;
    };
    let Expr::Field { object, field, .. } = callee.as_ref() else {
        return false;
    };
    field.name == "putIfAbsent"
        && state
            .signatures
            .implements_sdk(&state.infer(object), "dart:core", "Map")
            != Truth::No
}

/// The official lints report the invoked name, not the whole expression.
fn report_span(expr: &Expr) -> Span {
    match expr {
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field { field, .. } => field.span,
            other => *other.span(),
        },
        Expr::Field { field, .. } => field.span,
        other => *other.span(),
    }
}

/// Names of functions, methods, and getters annotated `@awaitNotRequired`.
#[derive(Default)]
struct AwaitNotRequired {
    names: HashSet<String>,
}

impl AwaitNotRequired {
    fn record(&mut self, annotations: &[Annotation], name: &Identifier) {
        if annotations.iter().any(|annotation| {
            annotation
                .name
                .last()
                .is_some_and(|segment| segment.name == "awaitNotRequired")
        }) {
            self.names.insert(name.name.clone());
        }
    }
}

impl Visitor for AwaitNotRequired {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.record(&node.annotations, &node.name);
        walk_function_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        match node {
            ClassMember::Method(method) => self.record(&method.annotations, &method.name),
            ClassMember::Getter(getter) => self.record(&getter.annotations, &getter.name),
            ClassMember::Field(field) => {
                for declarator in &field.declarators {
                    self.record(&field.annotations, &declarator.name);
                }
            }
            _ => {}
        }
        walk_class_member(self, node);
    }
}
