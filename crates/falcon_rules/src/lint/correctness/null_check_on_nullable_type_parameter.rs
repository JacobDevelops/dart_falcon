//! Flags a null assertion `x!` on a nullable type parameter `T?` whose
//! expected type is `T` or `T?` for a potentially nullable `T`, and
//! null-assert patterns whose matched value is a nullable type parameter.
//! Diagnostics underline only the terminal `!` of either AST assertion node.
//!
//! As in the analyzer's expected-type helper, only these positions have an
//! expected type:
//!
//! - a returned or yielded value (also awaited) of a function declaration
//!   with a declared return type (an `async` function's `Future` argument),
//!   or of a closure or untyped local function whose block body's returns
//!   infer `T` or `T?`; methods have none;
//! - a variable's initializer: its declared type, and for an untyped local or
//!   `for` variable the type inferred from `x!` itself;
//! - the value (also awaited) of a plain `=` assignment, except `x = x!` and
//!   `a.b = a.b!`;
//! - the right operand of an operator method, and an argument, including
//!   constructor-initializer `super(...)`/`this(...)` arguments; an awaited
//!   right operand or positional argument has none;
//! - an element of a list or set literal, a key or value of a map literal
//!   (through the map's collection `if` and `for` elements); an awaited or
//!   parenthesized map key expects the map's value type.
//!
//! Conditional branches, `??` operands, cascade targets, records, and
//! patterns have none. Inferred types never keep the `T & Object`
//! intersection of `x!`. Flow analysis promotes a local after its `x!`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use crate::lint::expressions::identifier_name;
use crate::lint::function_bodies::{BodyOwner, FunctionBodies};
use crate::lint::semantic_scope::{
    SemanticRuleVisitor, SemanticState, returned_expressions, visit_program,
};
use crate::lint::style::unnecessary_null_checks::declaration_initializers;

const RULE: &str = "null-check-on-nullable-type-parameter";
const MESSAGE: &str = "Avoid a null assertion on a nullable type parameter.";

#[derive(Default)]
pub struct NullCheckOnNullableTypeParameter;

impl Rule for NullCheckOnNullableTypeParameter {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            bodies: FunctionBodies::new(program),
            contextual: declaration_initializers(program).iter().map(key).collect(),
            initializers: HashSet::new(),
            expected: HashMap::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

fn key(span: &Span) -> (usize, usize) {
    (span.start, span.end)
}

/// The `x!` of `x!` or `await x!`, with whether it is awaited.
fn asserted(expression: &Expr) -> Option<(&Expr, bool)> {
    let expression = expression.unparenthesized();
    match expression {
        Expr::NullAssert { .. } => Some((expression, false)),
        Expr::Await { expr, .. } if matches!(expr.as_ref(), Expr::NullAssert { .. }) => {
            Some((expr.as_ref(), true))
        }
        _ => None,
    }
}

fn map_entry_asserts(element: &MapElement) -> bool {
    match element {
        MapElement::Entry(entry) => {
            asserted(&entry.key).is_some() || asserted(&entry.value).is_some()
        }
        MapElement::If {
            then_entry,
            else_entry,
            ..
        } => map_entry_asserts(then_entry) || else_entry.as_deref().is_some_and(map_entry_asserts),
        MapElement::For { entry, .. } => map_entry_asserts(entry),
        MapElement::Spread { .. } => false,
    }
}

/// The least upper bound of an inferred function's returned (or yielded)
/// types when it is a type parameter `T` or `T?`: every value is `T`, `T?`,
/// `T & Object` or `Null`, at least one `T` or `T?`. `Null` and an implicit
/// `null` only make it nullable, which keeps it matching `T?` operands.
fn returns_parameter(types: &[ResolvedType]) -> Option<ResolvedType> {
    let mut upper: Option<ResolvedType> = None;
    let mut id = None;
    for ty in types {
        let current = match ty {
            ResolvedType::Null => continue,
            ResolvedType::TypeParameter { id, nullable, .. } => {
                if upper
                    .as_ref()
                    .is_none_or(|upper| *nullable && !upper.nullable())
                {
                    upper = Some(ty.clone());
                }
                id
            }
            ResolvedType::NonNullTypeParameter { id, .. } => id,
            _ => return None,
        };
        if id.is_some_and(|id| id != current) {
            return None;
        }
        id = Some(current);
    }
    upper
}

/// The values of a generator body's own `yield` statements, without those of
/// nested functions.
struct Yields<'s, 'a> {
    state: &'s SemanticState<'a>,
    types: Vec<ResolvedType>,
    assertions: Vec<Span>,
    /// A `yield*` contributes an iterable's element type.
    delegated: bool,
}

impl Visitor for Yields<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Yield(statement) if statement.is_star => self.delegated = true,
            Stmt::Yield(statement) => {
                self.types.push(self.state.infer(&statement.value));
                if let Some((assertion, _)) = asserted(&statement.value) {
                    self.assertions.push(*assertion.span());
                }
            }
            Stmt::LocalFunc(_) => return,
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if !matches!(node, Expr::FuncExpr { .. }) {
            walk_expr(self, node);
        }
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies,
    /// `x!` / `await x!` whose expected type is the walker's context type.
    contextual: HashSet<(usize, usize)>,
    /// `x!` / `await x!` initializing a local or `for` variable.
    initializers: HashSet<(usize, usize)>,
    /// The expected type of each `x!` with one.
    expected: HashMap<(usize, usize), ResolvedType>,
}

impl Collector<'_> {
    fn mark_contextual(&mut self, expression: &Expr) {
        if asserted(expression).is_some() {
            self.contextual.insert(key(expression.span()));
        }
    }

    fn mark_initializers(&mut self, declaration: &LocalVarDecl) {
        for initializer in declaration
            .declarators
            .iter()
            .filter_map(|declarator| declarator.initializer.as_ref())
        {
            if asserted(initializer).is_some() {
                self.initializers.insert(key(initializer.span()));
            }
        }
    }

    /// The declared return (`yielded`: element) type of a function
    /// declaration's body; an awaited or `async` return expects the `Future`
    /// argument.
    fn returned(&mut self, value: &Expr, yielded: bool, state: &SemanticState<'_>) {
        let Some((assertion, awaited)) = asserted(value) else {
            return;
        };
        let Some(body) = self.bodies.enclosing(value.span()) else {
            return;
        };
        if !matches!(
            body.owner,
            BodyOwner::TopLevelFunction | BodyOwner::LocalFunction
        ) {
            return;
        }
        let Some(return_type) = &body.return_type else {
            return;
        };
        let ty = state
            .model
            .resolve_type_in(return_type, &state.type_parameters);
        let element = if yielded {
            (ty.interface("dart:core", "Iterable") || ty.interface("dart:async", "Stream"))
                .then(|| ty.type_argument(0))
                .flatten()
        } else if awaited || body.async_marker == AsyncMarker::Async {
            (ty.interface("dart:async", "Future") || ty.interface("dart:async", "FutureOr"))
                .then(|| ty.type_argument(0))
                .flatten()
        } else {
            Some(ty)
        };
        if let Some(element) = element {
            self.expected.insert(key(assertion.span()), element);
        }
    }

    /// A closure's or untyped local function's inferred return type (an
    /// `async` one's `Future` argument, a generator's element type) is the
    /// least upper bound of its returned or yielded values.
    fn inferred_returns(
        &mut self,
        body: &FunctionBody,
        async_marker: AsyncMarker,
        state: &SemanticState<'_>,
    ) {
        let (types, assertions) = match async_marker {
            AsyncMarker::Sync | AsyncMarker::Async => {
                let returns = returned_expressions(body);
                let assertions: Vec<_> = returns
                    .iter()
                    .filter_map(|value| asserted(value))
                    .map(|(assertion, _)| *assertion.span())
                    .collect();
                if assertions.is_empty() {
                    return;
                }
                let types: Vec<_> = returns.iter().map(|value| state.infer(value)).collect();
                (types, assertions)
            }
            AsyncMarker::SyncStar | AsyncMarker::AsyncStar => {
                let mut yields = Yields {
                    state,
                    types: Vec::new(),
                    assertions: Vec::new(),
                    delegated: false,
                };
                yields.visit_function_body(body);
                if yields.delegated {
                    return;
                }
                (yields.types, yields.assertions)
            }
        };
        if assertions.is_empty() {
            return;
        }
        if let Some(expected) = returns_parameter(&types) {
            for span in assertions {
                self.expected.insert(key(&span), expected.clone());
            }
        }
    }

    /// Elements of a collection literal expect its element type.
    fn collection(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let asserts = match node {
            Expr::List { elements, .. } | Expr::Set { elements, .. } => elements.iter().any(
                |item| matches!(item, CollectionElement::Expr(expression) if asserted(expression).is_some()),
            ),
            Expr::Map { elements, .. } => elements.iter().any(map_entry_asserts),
            _ => false,
        };
        if !asserts {
            return;
        }
        let literal = state.infer_with_context(node, state.expected());
        match node {
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                let Some(element) = literal.type_argument(0) else {
                    return;
                };
                for item in elements {
                    if let CollectionElement::Expr(expression) = item
                        && let Some((assertion, _)) = asserted(expression)
                    {
                        self.expected.insert(key(assertion.span()), element.clone());
                    }
                }
            }
            Expr::Map { elements, .. } => {
                if literal.arguments().len() == 2
                    && let (Some(key_type), Some(value_type)) =
                        (literal.type_argument(0), literal.type_argument(1))
                {
                    for element in elements {
                        self.map_entry(element, &key_type, &value_type);
                    }
                }
            }
            _ => {}
        }
    }

    /// The analyzer compares an entry's key with the asserted node itself, so
    /// an awaited or parenthesized key takes the value type.
    fn map_entry(&mut self, element: &MapElement, key_type: &ResolvedType, value: &ResolvedType) {
        match element {
            MapElement::Entry(entry) => {
                if let Some((assertion, awaited)) = asserted(&entry.key) {
                    let ty = if awaited || matches!(entry.key, Expr::Paren { .. }) {
                        value
                    } else {
                        key_type
                    };
                    self.expected.insert(key(assertion.span()), ty.clone());
                }
                if let Some((assertion, _)) = asserted(&entry.value) {
                    self.expected.insert(key(assertion.span()), value.clone());
                }
            }
            MapElement::If {
                then_entry,
                else_entry,
                ..
            } => {
                self.map_entry(then_entry, key_type, value);
                if let Some(entry) = else_entry {
                    self.map_entry(entry, key_type, value);
                }
            }
            MapElement::For { entry, .. } => self.map_entry(entry, key_type, value),
            MapElement::Spread { .. } => {}
        }
    }

    /// Record the walker's context of a contextual position, or for an
    /// untyped local the type inferred from `x!`.
    fn record_context(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let span = key(node.span());
        let initializer = self.initializers.contains(&span);
        if !initializer && !self.contextual.contains(&span) {
            return;
        }
        let Some((assertion, _)) = asserted(node) else {
            return;
        };
        let expected = match (state.expected(), assertion) {
            (Some(expected), _) => Some(expected.clone()),
            (None, Expr::NullAssert { operand, .. }) if initializer => {
                Some(state.infer(operand).non_null())
            }
            _ => None,
        };
        if let Some(expected) = expected {
            self.expected.insert(key(assertion.span()), expected);
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_call_arguments(
        &mut self,
        args: &ArgList,
        _positional: &[ResolvedType],
        _named: &HashMap<String, ResolvedType>,
        _state: &SemanticState<'_>,
    ) {
        // An awaited positional argument has no corresponding parameter; a
        // named one keeps its name's parameter.
        // Grouping does not change a positional assertion's parameter context.
        for argument in &args.positional {
            if matches!(argument.unparenthesized(), Expr::NullAssert { .. }) {
                self.mark_contextual(argument);
            }
        }
        for argument in &args.named {
            self.mark_contextual(&argument.value);
        }
    }

    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        match node {
            Stmt::Return(ReturnStmt {
                value: Some(value), ..
            }) => self.returned(value, false, state),
            Stmt::Yield(statement) => self.returned(&statement.value, true, state),
            Stmt::LocalVar(declaration) => self.mark_initializers(declaration),
            Stmt::LocalFunc(function) if function.return_type.is_none() => {
                self.inferred_returns(&function.body, function.async_marker, state);
            }
            Stmt::For(statement) => {
                if let Some(ForInit::VarDecl(declaration)) = &statement.header.init {
                    self.mark_initializers(declaration);
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if asserted(node).is_some()
            && self
                .bodies
                .enclosing(node.span())
                .is_some_and(|body| body.arrow_expression == Some(*node.span()))
        {
            self.returned(node, false, state);
        }
        self.record_context(node, state);
        match node {
            Expr::NullAssert { operand, span } => {
                let Some(expected) = self.expected.get(&key(span)) else {
                    return;
                };
                // An inferred `T & Object` stands for its demoted `T`.
                let expected = match expected {
                    ResolvedType::TypeParameter { id, .. } => {
                        Some((id, !expected.is_non_nullable()))
                    }
                    ResolvedType::NonNullTypeParameter { id, bound } => {
                        Some((id, !bound.is_non_nullable()))
                    }
                    _ => None,
                };
                if let Some((expected_id, potentially_nullable)) = expected
                    && potentially_nullable
                    && matches!(state.infer(operand), ResolvedType::TypeParameter {
                        id, nullable: true, ..
                    } if id == *expected_id)
                {
                    self.diagnostics.push(self.ctx.diagnostic(
                        RULE,
                        MESSAGE,
                        Span::new(span.end - 1, span.end),
                    ));
                }
            }
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } => {
                // `x = x!` / `a.b = a.b!` has no expected type.
                let same_name = match asserted(value) {
                    Some((Expr::NullAssert { operand, .. }, _)) => identifier_name(target)
                        .is_some_and(|name| identifier_name(operand) == Some(name)),
                    _ => false,
                };
                if !same_name {
                    self.mark_contextual(value);
                }
            }
            Expr::Binary { op, right, .. }
                if !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce)
                    && matches!(right.as_ref(), Expr::NullAssert { .. }) =>
            {
                self.mark_contextual(right);
            }
            Expr::Cascade { sections, .. } => {
                for operation in sections.iter().flat_map(|section| &section.ops) {
                    if let CascadeOp::Assign(_, AssignOp::Eq, value) = operation {
                        self.mark_contextual(value);
                    }
                }
            }
            Expr::FuncExpr {
                body, async_marker, ..
            } => self.inferred_returns(body, *async_marker, state),
            Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. } => {
                self.collection(node, state);
            }
            _ => {}
        }
    }

    fn visit_pattern(
        &mut self,
        node: &Pattern,
        matched: Option<&ResolvedType>,
        _state: &SemanticState<'_>,
    ) {
        if let Pattern::NullAssert { span, .. } = node
            && matches!(
                matched,
                Some(ResolvedType::TypeParameter { nullable: true, .. })
            )
        {
            self.diagnostics.push(self.ctx.diagnostic(
                RULE,
                MESSAGE,
                Span::new(span.end - 1, span.end),
            ));
        }
    }
}
