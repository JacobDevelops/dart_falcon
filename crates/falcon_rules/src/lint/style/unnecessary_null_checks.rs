//! Flags a null assertion `!` on a value that flows into a nullable context.
//!
//! When `x!` is passed to a `String?` parameter, assigned to an `int?`
//! variable, or returned from a function returning `Object?`, the context
//! accepts `null` anyway: the `!` only adds a runtime failure. The rule
//! fires on `!` whose value directly is:
//!
//! - the initializer of a variable or field declared with a nullable type;
//! - the right side of a plain `=` assignment to a nullable target other than
//!   a local variable or parameter (the `!` may promote those), unless the
//!   asserted operand is the assigned name itself (`x = x!`);
//! - an argument for a nullable parameter, except `Future.value(x!)` and
//!   `completer.complete(x!)`;
//! - returned from a top-level or local function (including async functions
//!   and top-level getters), or yielded to a nullable stream/iterable element,
//!   including `yield*` (matching the analyzer's element-type check);
//! - an element of a list or set literal, or a key or value of a map
//!   literal, whose element types are nullable;
//!
//! and on a null-assert pattern `p!` of a pattern assignment whose matched
//! value is nullable. In declarations, `for` loops, and `case` clauses the
//! `!` narrows the bound variables, so it is never flagged there.
//!
//! `dynamic` and `void` accept null too. Unresolved contexts stay silent.
//! Index writes and right-hand operator operands use their instantiated
//! parameter types, not the enclosing expression's result context.
//! Grouping parentheses preserve the nullable context of the inner assertion.
//!
//! Adopted from the official `unnecessary_null_checks` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::*;

use crate::lint::declarations::declaration_initializers as all_declaration_initializers;
use crate::lint::function_bodies::{BodyOwner, FunctionBodies};
use falcon_analyze::resolve::lexical_usage::locally_bound_identifiers;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UnnecessaryNullChecks;

impl Rule for UnnecessaryNullChecks {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            bodies: FunctionBodies::new(program),
            contextual: declaration_initializers(program),
            fixed: HashMap::new(),
            pattern_assignments: Vec::new(),
            locals: locally_bound_identifiers(program),
        }))
    }
}

/// Null assertions directly initializing top-level variables and fields; the
/// walker supplies their declared type as the expected type. Return the inner
/// assertion span when the initializer is parenthesized.
pub(crate) fn declaration_initializers(program: Program<'_>) -> Vec<Span> {
    all_declaration_initializers(program)
        .filter(|initializer| matches!(initializer.unparenthesized(), Expr::NullAssert(_)))
        .map(|initializer| initializer.unparenthesized().span())
        .collect()
}

fn null_assert_initializers<'a>(
    declarators: AstChildren<'a, VarDeclarator<'a>>,
) -> impl Iterator<Item = Span> + 'a {
    declarators
        .filter_map(|declarator| declarator.initializer())
        .filter(|initializer| matches!(initializer.unparenthesized(), Expr::NullAssert(_)))
        .map(|initializer| initializer.unparenthesized().span())
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies<'a>,
    /// `x!` nodes whose context type is the walker's expected type.
    contextual: Vec<Span>,
    /// `x!` collection elements with the element type of their literal.
    fixed: HashMap<(usize, usize), ResolvedType>,
    pattern_assignments: Vec<Span>,
    /// Start offsets of identifiers that resolve to a local or parameter.
    locals: HashSet<usize>,
}

/// Analyzer nullability includes `dynamic` and `void`, but not unresolved types.
fn explicitly_nullable(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Null | ResolvedType::Dynamic | ResolvedType::Void => true,
        ResolvedType::Interface { nullable, .. }
        | ResolvedType::TypeParameter { nullable, .. }
        | ResolvedType::Function { nullable, .. }
        | ResolvedType::Record { nullable, .. } => *nullable,
        ResolvedType::Unknown | ResolvedType::Never | ResolvedType::NonNullTypeParameter { .. } => {
            false
        }
    }
}

fn key(span: &Span) -> (usize, usize) {
    (span.start, span.end)
}

fn is_null_assert(expression: Expr<'_>) -> bool {
    matches!(expression.unparenthesized(), Expr::NullAssert(_))
}

fn is_future_value<'a>(node: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let future =
        |identity: &falcon_analyze::DeclarationIdentity| identity.is_sdk("dart:async", "Future");
    match node.unparenthesized() {
        Expr::New(expression) => {
            expression
                .constructor_name()
                .is_some_and(|constructor| constructor.name() == "value")
                && matches!(state.model.resolve_type(expression.dart_type()),
                    ResolvedType::Interface { identity, .. } if future(&identity))
        }
        Expr::Call(call) => match call.callee().unparenthesized() {
            Expr::Field(access) if access.field().name() == "value" => {
                falcon_analyze::expression_segments(access.object(), true)
                    .filter(|segments| {
                        segments
                            .first()
                            .is_some_and(|head| !state.environment.is_bound(head))
                    })
                    .and_then(|segments| state.model.resolve_dotted(&segments))
                    .is_some_and(|identity| future(&identity))
            }
            _ => false,
        },
        _ => false,
    }
}

/// `completer.complete(...)` on a `dart:async` `Completer`.
fn is_completer_complete<'a>(callee: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let Expr::Field(access) = callee.unparenthesized() else {
        return false;
    };
    access.field().name() == "complete"
        && matches!(state.infer(access.object()), ResolvedType::Interface { identity, .. }
            if identity.is_sdk("dart:async", "Completer"))
}

#[derive(Clone, Copy)]
enum ValueContext {
    Return,
    Yield,
}

impl<'a> Collector<'a> {
    fn report(&mut self, span: &Span) {
        self.diags.push(self.ctx.diagnostic(
            "unnecessary-null-checks",
            "Unnecessary use of a null check ('!').",
            DiagSpan {
                start: span.end - 1,
                end: span.end,
            },
        ));
    }

    fn contextual_arguments(&mut self, args: ArgList<'a>) {
        self.contextual.extend(
            args.positional()
                .chain(args.named().map(|argument| argument.value()))
                .filter(|argument| is_null_assert(*argument))
                .map(|argument| argument.unparenthesized().span()),
        );
    }

    /// Only function declarations have independently declared return types;
    /// method bodies and inferred closures are excluded by the official lint.
    fn return_context(
        &self,
        span: &Span,
        state: &SemanticState<'a>,
        context: ValueContext,
        awaited: bool,
    ) -> Option<ResolvedType> {
        let body = self.bodies.enclosing(span)?;
        if !matches!(
            body.owner,
            BodyOwner::TopLevelFunction | BodyOwner::LocalFunction
        ) && !(body.owner == BodyOwner::Getter && state.enclosing_type.is_none())
        {
            return None;
        }
        let ty = state
            .model
            .resolve_type_in(body.return_type?, &state.type_parameters);
        if matches!(context, ValueContext::Yield) {
            (ty.interface("dart:core", "Iterable") || ty.interface("dart:async", "Stream"))
                .then(|| ty.type_argument(0))
                .flatten()
        } else if awaited || body.async_marker == AsyncMarker::Async {
            (ty.interface("dart:async", "Future") || ty.interface("dart:async", "FutureOr"))
                .then(|| ty.type_argument(0))
                .flatten()
        } else {
            Some(ty)
        }
    }

    fn fix_return(&mut self, value: Expr<'a>, state: &SemanticState<'a>, context: ValueContext) {
        let (value, awaited) = match value.unparenthesized() {
            Expr::Await(expression) => (expression.expr().unparenthesized(), true),
            value => (value, false),
        };
        if is_null_assert(value)
            && let Some(ty) = self.return_context(&value.span(), state, context, awaited)
        {
            self.fixed.insert(key(&value.span()), ty);
        }
    }

    fn fix_elements(
        &mut self,
        elements: AstChildren<'a, CollectionElement<'a>>,
        element: &ResolvedType,
    ) {
        for item in elements {
            if let CollectionElement::Expr(expression) = item
                && is_null_assert(expression)
            {
                self.fixed
                    .insert(key(&expression.unparenthesized().span()), element.clone());
            }
        }
    }

    fn fix_map_entry(
        &mut self,
        item: MapElement<'a>,
        key_type: &ResolvedType,
        value: &ResolvedType,
    ) {
        match item {
            MapElement::Entry(entry) => {
                if is_null_assert(entry.key()) {
                    self.fixed
                        .insert(key(&entry.key().unparenthesized().span()), key_type.clone());
                }
                if is_null_assert(entry.value()) {
                    self.fixed
                        .insert(key(&entry.value().unparenthesized().span()), value.clone());
                }
            }
            MapElement::If(element) => {
                self.fix_map_entry(element.then_entry(), key_type, value);
                if let Some(entry) = element.else_entry() {
                    self.fix_map_entry(entry, key_type, value);
                }
            }
            MapElement::For(element) => self.fix_map_entry(element.entry(), key_type, value),
            MapElement::Spread(_) => {}
        }
    }

    /// Type arguments of a collection literal: explicit, else its context type's.
    fn literal_arguments(
        explicit: impl Iterator<Item = DartType<'a>> + Clone,
        sdk_names: &[&str],
        state: &SemanticState<'a>,
    ) -> Option<std::sync::Arc<[ResolvedType]>> {
        if explicit.clone().next().is_some() {
            return Some(
                explicit
                    .map(|ty| state.model.resolve_type_in(ty, &state.type_parameters))
                    .collect(),
            );
        }
        match state.expected()? {
            ResolvedType::Interface {
                identity,
                arguments,
                ..
            } if sdk_names
                .iter()
                .any(|name| identity.is_sdk("dart:core", name)) =>
            {
                Some(arguments.clone())
            }
            _ => None,
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        match node {
            Stmt::LocalVar(declaration) => {
                self.contextual
                    .extend(null_assert_initializers(declaration.declarators()));
            }
            Stmt::Return(statement) => {
                if let Some(value) = statement.value() {
                    self.fix_return(value, state, ValueContext::Return);
                }
            }
            Stmt::Yield(statement) => {
                self.fix_return(statement.value(), state, ValueContext::Yield);
            }
            Stmt::PatternAssign(assignment) => {
                self.pattern_assignments.push(assignment.pattern().span());
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(node, Expr::Paren(_)) {
            return;
        }
        match node {
            Expr::NullAssert(assertion) => {
                let span = assertion.span();
                let context = if let Some(ty) = self.fixed.get(&key(&span)) {
                    Some(ty.clone())
                } else if self.contextual.contains(&span) {
                    state.expected().cloned()
                } else if self
                    .bodies
                    .enclosing(&span)
                    .is_some_and(|body| body.arrow_expression == Some(span))
                {
                    self.return_context(&span, state, ValueContext::Return, false)
                } else {
                    None
                };
                if context.as_ref().is_some_and(explicitly_nullable) {
                    self.report(&span);
                }
            }
            Expr::Await(_)
                if self
                    .bodies
                    .enclosing(&node.span())
                    .is_some_and(|body| body.arrow_expression == Some(node.span())) =>
            {
                self.fix_return(node, state, ValueContext::Return);
            }
            Expr::Binary(binary)
                if is_null_assert(binary.right())
                    && !matches!(
                        binary.op(),
                        BinaryOp::NullCoalesce | BinaryOp::And | BinaryOp::Or
                    ) =>
            {
                self.contextual
                    .push(binary.right().unparenthesized().span());
            }
            Expr::Assign(assignment)
                if assignment.op() == AssignOp::Eq && is_null_assert(assignment.value()) =>
            {
                let target = assignment.target();
                let value = assignment.value();
                // As in the official lint, the `!` may promote a local or a
                // parameter. Fields never qualify: a promotable field is final
                // and not `late`, so it cannot be assigned.
                let promotable = match (target.unparenthesized(), value.unparenthesized()) {
                    (Expr::Ident(name), Expr::NullAssert(assertion)) => {
                        self.locals.contains(&name.span().start)
                            || matches!(assertion.operand().unparenthesized(), Expr::Ident(asserted) if asserted.name() == name.name())
                    }
                    _ => false,
                };
                if !promotable {
                    self.contextual.push(value.unparenthesized().span());
                }
            }
            Expr::Call(call)
                if call.args().positional().any(is_null_assert)
                    || call
                        .args()
                        .named()
                        .any(|argument| is_null_assert(argument.value())) =>
            {
                let callee = call.callee();
                let type_args = call.type_args();
                let args = call.args();
                if !is_future_value(node, state) && !is_completer_complete(callee, state) {
                    // A call's result context must not widen the argument type
                    // (notably dart:math min/max in nullable assignments).
                    if let Some(mut signature) = state.call_signature(callee, type_args, args, None)
                    {
                        for (argument, expected) in args.positional().zip(signature.positional) {
                            if is_null_assert(argument) {
                                self.fixed
                                    .insert(key(&argument.unparenthesized().span()), expected);
                            }
                        }
                        for argument in args.named() {
                            if is_null_assert(argument.value())
                                && let Some(expected) =
                                    signature.named.remove(argument.name().name())
                            {
                                self.fixed.insert(
                                    key(&argument.value().unparenthesized().span()),
                                    expected,
                                );
                            }
                        }
                    }
                    self.contextual_arguments(args);
                }
            }
            Expr::New(expression) => {
                if !is_future_value(node, state) {
                    self.contextual_arguments(expression.args());
                }
            }
            Expr::List(_) | Expr::Set(_) => {
                let (type_arg, elements) = match node {
                    Expr::List(list) => (list.type_arg(), list.elements()),
                    Expr::Set(set) => (set.type_arg(), set.elements()),
                    _ => unreachable!(),
                };
                let explicit = type_arg.into_iter();
                let names: &[&str] = if matches!(node, Expr::List(_)) {
                    &["List", "Iterable"]
                } else {
                    &["Set", "Iterable"]
                };
                if let Some(arguments) = Self::literal_arguments(explicit, names, state)
                    && let [element] = arguments.as_ref()
                {
                    self.fix_elements(elements, element);
                }
            }
            Expr::Map(map) => {
                if let Some(arguments) = Self::literal_arguments(map.type_args(), &["Map"], state)
                    && let [key_type, value] = arguments.as_ref()
                {
                    for element in map.elements() {
                        self.fix_map_entry(element, key_type, value);
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        matched: Option<&ResolvedType>,
        _state: &SemanticState<'a>,
    ) {
        if let Pattern::NullAssert(assertion) = node
            && self.pattern_assignments.iter().any(|assignment| {
                assignment.start <= assertion.span().start && assertion.span().end <= assignment.end
            })
            && matched.is_some_and(explicitly_nullable)
        {
            self.report(&assertion.span());
        }
    }
}
