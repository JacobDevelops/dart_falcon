//! Flags type annotations on closure parameters whose types come from context.
//!
//! When a function expression is passed where a function type is expected,
//! its parameter types are inferred from that type, so annotating them is
//! redundant. Closures without a known function-typed context are left alone
//! because their annotations carry information. Like the official lint, a
//! closure that is an operand, an `await` operand, a switch expression arm,
//! a record literal field, or a direct collection literal element/key/value
//! (including collection `if`/`for`) has no context here. This does not suppress
//! independently contextualized callbacks nested inside those expressions.
//! A returned closure has context only in an `async` body returning `Future<F>`
//! for a function type `F`. A conditional branch or assignment RHS takes its
//! enclosing expression's own static type as its context, including function
//! types obtained by joining variant signatures. Joins retain the shared
//! optional positional prefix when required arity agrees. Different required
//! arities join to general `Function`, unless a concrete downward function
//! context provides a shared signature.
//!
//! ```dart
//! names.map((String name) => name.length); // flagged
//! names.map((name) => name.length);        // ok
//! ```
//!
//! Adopted from the official `avoid_types_on_closure_parameters` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidTypesOnClosureParameters;

impl Rule for AvoidTypesOnClosureParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut returns = ReturnedClosures {
            model: &state.model,
            frames: Vec::new(),
            returned: HashMap::new(),
        };
        returns.visit_program(program);
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            uncontextual: HashSet::new(),
            returned: returns.returned,
            future_closures: HashSet::new(),
            expression_contexts: HashMap::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

/// The callable a `return` statement leaves.
#[derive(Clone, Copy)]
enum ReturnFrame {
    /// A declaration; `true` when it is `async` with a declared `Future`
    /// return type.
    Declared(bool),
    /// An `async` function expression, whose return type comes from context.
    AsyncClosure(*const Expr),
}

/// Maps each closure that is a `return` value to its enclosing callable.
struct ReturnedClosures<'a, 'model> {
    model: &'a SemanticModel<'model>,
    frames: Vec<ReturnFrame>,
    returned: HashMap<*const Expr, ReturnFrame>,
}

impl ReturnedClosures<'_, '_> {
    fn declared(
        &mut self,
        marker: AsyncMarker,
        return_type: Option<&DartType>,
        walk: impl FnOnce(&mut Self),
    ) {
        let future = marker == AsyncMarker::Async
            && return_type.is_some_and(|ty| {
                self.model
                    .resolve_type(ty)
                    .interface("dart:async", "Future")
            });
        self.with_frame(ReturnFrame::Declared(future), walk);
    }

    fn with_frame(&mut self, frame: ReturnFrame, walk: impl FnOnce(&mut Self)) {
        self.frames.push(frame);
        walk(self);
        self.frames.pop();
    }
}

impl Visitor for ReturnedClosures<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.declared(node.async_marker, node.return_type.as_ref(), |v| {
            visitor::walk_function_decl(v, node)
        });
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.declared(node.async_marker, node.return_type.as_ref(), |v| {
            visitor::walk_method_decl(v, node)
        });
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.declared(node.async_marker, node.return_type.as_ref(), |v| {
            visitor::walk_getter_decl(v, node)
        });
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.with_frame(ReturnFrame::Declared(false), |v| {
            visitor::walk_setter_decl(v, node)
        });
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.with_frame(ReturnFrame::Declared(false), |v| {
            visitor::walk_constructor_decl(v, node)
        });
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        match node {
            ClassMember::Operator(operator) => {
                self.declared(operator.async_marker, operator.return_type.as_ref(), |v| {
                    visitor::walk_class_member(v, node)
                });
            }
            _ => visitor::walk_class_member(self, node),
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalFunc(function) => {
                self.declared(function.async_marker, function.return_type.as_ref(), |v| {
                    visitor::walk_stmt(v, node)
                });
            }
            Stmt::Return(ReturnStmt {
                value: Some(value @ Expr::FuncExpr { .. }),
                ..
            }) => {
                if let Some(frame) = self.frames.last() {
                    self.returned.insert(value, *frame);
                }
                visitor::walk_stmt(self, node);
            }
            _ => visitor::walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        let Expr::FuncExpr { async_marker, .. } = node else {
            return visitor::walk_expr(self, node);
        };
        let frame = if *async_marker == AsyncMarker::Async {
            ReturnFrame::AsyncClosure(node)
        } else {
            ReturnFrame::Declared(false)
        };
        self.with_frame(frame, |v| visitor::walk_expr(v, node));
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Expressions in positions the official lint gives no context type,
    /// recorded by their parent before the walk reaches them.
    uncontextual: HashSet<*const Expr>,
    /// Closures that are `return` values, with their enclosing callable.
    returned: HashMap<*const Expr, ReturnFrame>,
    /// `async` closures whose context returns a `Future`.
    future_closures: HashSet<*const Expr>,
    /// Branches and assignment RHSs take their enclosing expression's static
    /// type rather than its downward context in the official approximation.
    expression_contexts: HashMap<*const Expr, bool>,
}

impl Collector<'_> {
    /// Whether the official lint sees a function-typed context for `node`.
    fn has_function_context(&self, node: &Expr, expected: Option<&ResolvedType>) -> bool {
        if let Some(function) = self.expression_contexts.get(&(node as *const Expr)) {
            return *function;
        }
        let Some(frame) = self.returned.get(&(node as *const Expr)) else {
            return matches!(expected, Some(ResolvedType::Function { .. }));
        };
        // A returned value's context is the declared or contextual return type
        // only when an `async` body returns `Future<F>`; the walker gives it
        // the `FutureOr<F>` context then.
        let future = match frame {
            ReturnFrame::Declared(future) => *future,
            ReturnFrame::AsyncClosure(closure) => self.future_closures.contains(closure),
        };
        future
            && matches!(expected, Some(ResolvedType::Interface { identity, arguments, .. })
                if identity.is_sdk("dart:async", "FutureOr")
                    && matches!(arguments.first(), Some(ResolvedType::Function { .. })))
    }

    fn uncontextual_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::Expr(expr) | CollectionElement::NullAware { expr, .. } => {
                self.uncontextual.insert(expr);
            }
            CollectionElement::Spread { .. } => {}
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                self.uncontextual_element(then_elem);
                if let Some(element) = else_elem {
                    self.uncontextual_element(element);
                }
            }
            CollectionElement::For { element, .. } => self.uncontextual_element(element),
        }
    }

    fn uncontextual_entry(&mut self, element: &MapElement) {
        match element {
            MapElement::Entry(entry) => {
                self.uncontextual.insert(&entry.key);
                self.uncontextual.insert(&entry.value);
            }
            MapElement::Spread { .. } => {}
            MapElement::If {
                then_entry,
                else_entry,
                ..
            } => {
                self.uncontextual_entry(then_entry);
                if let Some(entry) = else_entry {
                    self.uncontextual_entry(entry);
                }
            }
            MapElement::For { entry, .. } => self.uncontextual_entry(entry),
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Binary { left, right, .. } => {
                self.uncontextual.insert(&**left);
                self.uncontextual.insert(&**right);
            }
            Expr::Await { expr, .. } => {
                self.uncontextual.insert(&**expr);
            }
            Expr::Switch { arms, .. } => {
                self.uncontextual
                    .extend(arms.iter().map(|arm| &arm.body as *const Expr));
            }
            Expr::Record { fields, .. } => {
                self.uncontextual
                    .extend(fields.iter().map(|field| &field.value as *const Expr));
            }
            // The official context approximation does not reach collection
            // literal elements, entries, or the elements under `if`/`for`.
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                elements
                    .iter()
                    .for_each(|element| self.uncontextual_element(element));
            }
            Expr::Map { elements, .. } => {
                elements
                    .iter()
                    .for_each(|element| self.uncontextual_entry(element));
            }
            Expr::Assign { value, .. } => {
                let function = matches!(
                    state.infer_with_context(node, state.expected()),
                    ResolvedType::Function { .. }
                );
                self.expression_contexts.insert(&**value, function);
            }
            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => {
                let function = matches!(
                    state.infer_with_context(node, state.expected()),
                    ResolvedType::Function { .. }
                );
                self.expression_contexts.insert(&**then_expr, function);
                self.expression_contexts.insert(&**else_expr, function);
            }
            _ => {}
        }
        let Expr::FuncExpr {
            params,
            async_marker,
            ..
        } = node
        else {
            return;
        };
        if *async_marker == AsyncMarker::Async
            && matches!(state.expected(), Some(ResolvedType::Function { return_type, .. })
                if matches!(&**return_type, ResolvedType::Interface { identity, .. }
                    if identity.is_sdk("dart:async", "Future")))
        {
            self.future_closures.insert(node);
        }
        if self.uncontextual.contains(&(node as *const Expr))
            || !self.has_function_context(node, state.expected())
        {
            return;
        }
        for param in &params.params {
            // A function-typed formal (`int f()`) is reported whole; otherwise
            // only a named type other than `dynamic` is redundant.
            let span = if param.function_params.is_some() {
                param.span
            } else {
                match &param.param_type {
                    Some(ty @ DartType::Named(named))
                        if !matches!(
                            state.model.resolve_type_in(ty, &state.type_parameters),
                            ResolvedType::Dynamic
                        ) =>
                    {
                        named.span
                    }
                    Some(DartType::Void { span }) => *span,
                    Some(
                        DartType::Named(_)
                        | DartType::Dynamic { .. }
                        | DartType::Function(_)
                        | DartType::Record(_),
                    )
                    | None => continue,
                }
            };
            self.diags.push(self.ctx.diagnostic(
                "avoid-types-on-closure-parameters",
                "Unnecessary type annotation on a function expression parameter.",
                span,
            ));
        }
    }
}
