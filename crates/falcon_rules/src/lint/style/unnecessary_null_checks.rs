//! Flags a null assertion `!` on a value that flows into a nullable context.
//!
//! When `x!` is passed to a `String?` parameter, assigned to an `int?`
//! variable, or returned from a function returning `Object?`, the context
//! accepts `null` anyway: the `!` only adds a runtime failure. The rule
//! fires on `!` whose value directly is:
//!
//! - the initializer of a variable or field declared with a nullable type;
//! - the right side of a plain `=` assignment to a nullable non-local target
//!   (locals, parameters, and private fields are skipped because the `!`
//!   may promote them);
//! - an argument for a nullable parameter, except `Future.value(x!)` and
//!   `completer.complete(x!)`;
//! - returned (`return x!` or `=> x!`) from a synchronous function with a
//!   declared nullable return type (method bodies are not checked, as in the
//!   official lint, and neither are closures: a function literal's return
//!   type is inferred from its body, not taken from its context);
//! - an element of a list or set literal, or a key or value of a map
//!   literal, whose element types are nullable;
//!
//! and on a null-assert pattern `p!` of a pattern assignment whose matched
//! value is nullable. In declarations, `for` loops, and `case` clauses the
//! `!` narrows the bound variables, so it is never flagged there.
//!
//! falcon needs the context type proven nullable; unresolved contexts are
//! skipped. `!` under `await`, on `yield`ed values, and on the right operand of
//! user-defined operators is not checked.
//!
//! Adopted from the official `unnecessary_null_checks` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;

use crate::lint::declarations::members_of;
use crate::lint::function_bodies::{BodyOwner, FunctionBodies};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnnecessaryNullChecks;

impl Rule for UnnecessaryNullChecks {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            bodies: FunctionBodies::new(program),
            contextual: declaration_initializers(program),
            fixed: HashMap::new(),
            pattern_assignments: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

/// Initializers of top-level variables and fields; the walker supplies their
/// declared type as the expected type.
fn declaration_initializers(program: &Program) -> Vec<Span> {
    let mut spans = Vec::new();
    for declaration in &program.declarations {
        if let TopLevelDecl::Variable(variable) = declaration {
            spans.extend(null_assert_initializers(&variable.declarators));
        }
        for member in members_of(declaration).unwrap_or_default() {
            if let ClassMember::Field(field) = member {
                spans.extend(null_assert_initializers(&field.declarators));
            }
        }
    }
    spans
}

fn null_assert_initializers(declarators: &[VarDeclarator]) -> impl Iterator<Item = Span> + '_ {
    declarators
        .iter()
        .filter_map(|declarator| declarator.initializer.as_ref())
        .filter(|initializer| matches!(initializer, Expr::NullAssert { .. }))
        .map(|initializer| *initializer.span())
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    bodies: FunctionBodies,
    /// `x!` nodes whose context type is the walker's expected type.
    contextual: Vec<Span>,
    /// `x!` collection elements with the element type of their literal.
    fixed: HashMap<(usize, usize), ResolvedType>,
    pattern_assignments: Vec<Span>,
}

/// Whether `ty` is written nullable; `dynamic` and unresolved types are not proof.
fn explicitly_nullable(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Null => true,
        ResolvedType::Interface { nullable, .. }
        | ResolvedType::TypeParameter { nullable, .. }
        | ResolvedType::Function { nullable, .. }
        | ResolvedType::Record { nullable, .. } => *nullable,
        ResolvedType::Unknown
        | ResolvedType::Dynamic
        | ResolvedType::Void
        | ResolvedType::Never => false,
    }
}

fn key(span: &Span) -> (usize, usize) {
    (span.start, span.end)
}

fn is_null_assert(expression: &Expr) -> bool {
    matches!(expression, Expr::NullAssert { .. })
}

fn is_future_value(node: &Expr, state: &SemanticState<'_>) -> bool {
    let future =
        |identity: &falcon_analyze::DeclarationIdentity| identity.is_sdk("dart:async", "Future");
    match node {
        Expr::New {
            dart_type,
            constructor_name: Some(constructor),
            ..
        } => {
            constructor.name == "value"
                && matches!(state.model.resolve_type(dart_type),
                    ResolvedType::Interface { identity, .. } if future(&identity))
        }
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field { object, field, .. } => match object.as_ref() {
                Expr::Ident(name) => {
                    field.name == "value"
                        && !state.environment.is_bound(&name.name)
                        && state
                            .model
                            .resolve_name(name.name.as_str())
                            .is_some_and(|identity| future(&identity))
                }
                _ => false,
            },
            _ => false,
        },
        _ => false,
    }
}

/// `completer.complete(...)` on a `dart:async` `Completer`.
fn is_completer_complete(callee: &Expr, state: &SemanticState<'_>) -> bool {
    let Expr::Field { object, field, .. } = callee else {
        return false;
    };
    field.name == "complete"
        && matches!(state.infer(object), ResolvedType::Interface { identity, .. }
            if identity.is_sdk("dart:async", "Completer"))
}

impl Collector<'_> {
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

    fn contextual_arguments(&mut self, args: &ArgList) {
        self.contextual.extend(
            args.positional
                .iter()
                .chain(args.named.iter().map(|argument| &argument.value))
                .filter(|argument| is_null_assert(argument))
                .map(|argument| *argument.span()),
        );
    }

    /// Whether `x!` at `span` is returned from a synchronous top-level or local
    /// function. Closures are excluded: their inferred return type comes from
    /// the returned expressions, so `() => x!` returns a non-nullable type even
    /// where a nullable function type is expected.
    fn returned_from_function(&self, span: &Span) -> bool {
        self.bodies.enclosing(span).is_some_and(|body| {
            matches!(
                body.owner,
                BodyOwner::TopLevelFunction | BodyOwner::LocalFunction
            ) && body.async_marker == AsyncMarker::Sync
        })
    }

    fn fix_elements(&mut self, elements: &[CollectionElement], element: &ResolvedType) {
        for item in elements {
            if let CollectionElement::Expr(expression) = item
                && is_null_assert(expression)
            {
                self.fixed.insert(key(expression.span()), element.clone());
            }
        }
    }

    fn fix_map_entry(&mut self, item: &MapElement, key_type: &ResolvedType, value: &ResolvedType) {
        match item {
            MapElement::Entry(entry) => {
                if is_null_assert(&entry.key) {
                    self.fixed.insert(key(entry.key.span()), key_type.clone());
                }
                if is_null_assert(&entry.value) {
                    self.fixed.insert(key(entry.value.span()), value.clone());
                }
            }
            MapElement::If {
                then_entry,
                else_entry,
                ..
            } => {
                self.fix_map_entry(then_entry, key_type, value);
                if let Some(entry) = else_entry {
                    self.fix_map_entry(entry, key_type, value);
                }
            }
            MapElement::For { entry, .. } => self.fix_map_entry(entry, key_type, value),
            MapElement::Spread { .. } => {}
        }
    }

    /// Type arguments of a collection literal: explicit, else its context type's.
    fn literal_arguments(
        explicit: &[DartType],
        sdk_names: &[&str],
        state: &SemanticState<'_>,
    ) -> Option<Vec<ResolvedType>> {
        if !explicit.is_empty() {
            return Some(
                explicit
                    .iter()
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

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, _state: &SemanticState<'_>) {
        match node {
            Stmt::LocalVar(declaration) => {
                self.contextual
                    .extend(null_assert_initializers(&declaration.declarators));
            }
            Stmt::Return(ReturnStmt {
                value: Some(value), ..
            }) if is_null_assert(value) && self.returned_from_function(value.span()) => {
                self.contextual.push(*value.span());
            }
            Stmt::PatternAssign(assignment) => {
                self.pattern_assignments.push(*assignment.pattern.span());
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::NullAssert { span, .. } => {
                let context = if let Some(ty) = self.fixed.get(&key(span)) {
                    Some(ty.clone())
                } else if self.contextual.contains(span)
                    || self.bodies.enclosing(span).is_some_and(|body| {
                        body.arrow_expression == Some(*span) && self.returned_from_function(span)
                    })
                {
                    state.expected().cloned()
                } else {
                    None
                };
                if context.as_ref().is_some_and(explicitly_nullable) {
                    self.report(span);
                }
            }
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } if is_null_assert(value) => {
                let promotable = match target.as_ref() {
                    Expr::Ident(name) => {
                        state.environment.is_bound(&name.name) || name.name.starts_with('_')
                    }
                    _ => false,
                };
                if !promotable {
                    self.contextual.push(*value.span());
                }
            }
            Expr::Call { callee, args, .. } => {
                if !is_future_value(node, state) && !is_completer_complete(callee, state) {
                    self.contextual_arguments(args);
                }
            }
            Expr::New { args, .. } => {
                if !is_future_value(node, state) {
                    self.contextual_arguments(args);
                }
            }
            Expr::List {
                type_arg, elements, ..
            }
            | Expr::Set {
                type_arg, elements, ..
            } => {
                let explicit = type_arg.as_slice();
                let names: &[&str] = if matches!(node, Expr::List { .. }) {
                    &["List", "Iterable"]
                } else {
                    &["Set", "Iterable"]
                };
                if let Some(arguments) = Self::literal_arguments(explicit, names, state)
                    && let [element] = arguments.as_slice()
                {
                    self.fix_elements(elements, element);
                }
            }
            Expr::Map {
                type_args,
                elements,
                ..
            } => {
                if let Some(arguments) = Self::literal_arguments(type_args, &["Map"], state)
                    && let [key_type, value] = arguments.as_slice()
                {
                    for element in elements {
                        self.fix_map_entry(element, key_type, value);
                    }
                }
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
            && self
                .pattern_assignments
                .iter()
                .any(|assignment| assignment.start <= span.start && span.end <= assignment.end)
            && matched.is_some_and(explicitly_nullable)
        {
            self.report(span);
        }
    }
}
