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
//! Parenthesizing an operand (including the fallback of `??`) does not add context.
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

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{self as visitor, Visitor};
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidTypesOnClosureParameters;

impl Rule for AvoidTypesOnClosureParameters {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let mut returns = ReturnedClosures {
            model: &model,
            frames: Vec::new(),
            returned: HashMap::new(),
        };
        returns.visit_program(program);
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            uncontextual: HashSet::new(),
            returned: returns.returned,
            future_closures: HashSet::new(),
            expression_contexts: HashMap::new(),
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

/// The callable a `return` statement leaves.
#[derive(Clone, Copy)]
enum ReturnFrame {
    /// A declaration; `true` when it is `async` with a declared `Future`
    /// return type.
    Declared(bool),
    /// An `async` function expression, whose return type comes from context.
    AsyncClosure(NodeId),
}

/// Maps each closure that is a `return` value to its enclosing callable.
struct ReturnedClosures<'a, 'model> {
    model: &'a SemanticModel<'model>,
    frames: Vec<ReturnFrame>,
    returned: HashMap<NodeId, ReturnFrame>,
}

impl ReturnedClosures<'_, '_> {
    fn declared(
        &mut self,
        marker: AsyncMarker,
        return_type: Option<DartType<'_>>,
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

impl<'a> Visitor<'a> for ReturnedClosures<'_, '_> {
    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) {
        self.declared(node.async_marker(), node.return_type(), |v| {
            visitor::walk_function_decl(v, node)
        });
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) {
        self.declared(node.async_marker(), node.return_type(), |v| {
            visitor::walk_method_decl(v, node)
        });
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) {
        self.declared(node.async_marker(), node.return_type(), |v| {
            visitor::walk_getter_decl(v, node)
        });
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) {
        self.with_frame(ReturnFrame::Declared(false), |v| {
            visitor::walk_setter_decl(v, node)
        });
    }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) {
        self.with_frame(ReturnFrame::Declared(false), |v| {
            visitor::walk_constructor_decl(v, node)
        });
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) {
        match node {
            ClassMember::Operator(operator) => {
                self.declared(operator.async_marker(), operator.return_type(), |v| {
                    visitor::walk_class_member(v, node)
                });
            }
            _ => visitor::walk_class_member(self, node),
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        match node {
            Stmt::LocalFunc(function) => {
                self.declared(function.async_marker(), function.return_type(), |v| {
                    visitor::walk_stmt(v, node)
                });
            }
            Stmt::Return(statement) => {
                if let Some(frame) = self.frames.last()
                    && let Some(value) = statement.value()
                    && let value @ Expr::FuncExpr(_) = value.unparenthesized()
                {
                    self.returned.insert(value.syntax().id(), *frame);
                }
                visitor::walk_stmt(self, node);
            }
            _ => visitor::walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        let Expr::FuncExpr(closure) = node else {
            return visitor::walk_expr(self, node);
        };
        let frame = if closure.async_marker() == AsyncMarker::Async {
            ReturnFrame::AsyncClosure(node.syntax().id())
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
    uncontextual: HashSet<NodeId>,
    /// Closures that are `return` values, with their enclosing callable.
    returned: HashMap<NodeId, ReturnFrame>,
    /// `async` closures whose context returns a `Future`.
    future_closures: HashSet<NodeId>,
    /// Branches and assignment RHSs take their enclosing expression's static
    /// type rather than its downward context in the official approximation.
    expression_contexts: HashMap<NodeId, bool>,
}

impl Collector<'_> {
    /// Whether the official lint sees a function-typed context for `node`.
    fn has_function_context(&self, node: Expr<'_>, expected: Option<&ResolvedType>) -> bool {
        let id = node.syntax().id();
        if let Some(function) = self.expression_contexts.get(&id) {
            return *function;
        }
        let Some(frame) = self.returned.get(&id) else {
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

    fn uncontextual_element(&mut self, element: CollectionElement<'_>) {
        match element {
            CollectionElement::Expr(expr) => {
                self.uncontextual.insert(expr.syntax().id());
            }
            CollectionElement::NullAware(element) => {
                self.uncontextual.insert(element.expr().syntax().id());
            }
            CollectionElement::Spread(_) => {}
            CollectionElement::If(element) => {
                self.uncontextual_element(element.then_elem());
                if let Some(element) = element.else_elem() {
                    self.uncontextual_element(element);
                }
            }
            CollectionElement::For(element) => self.uncontextual_element(element.element()),
        }
    }

    fn uncontextual_entry(&mut self, element: MapElement<'_>) {
        match element {
            MapElement::Entry(entry) => {
                self.uncontextual.insert(entry.key().syntax().id());
                self.uncontextual.insert(entry.value().syntax().id());
            }
            MapElement::Spread(_) => {}
            MapElement::If(element) => {
                self.uncontextual_entry(element.then_entry());
                if let Some(entry) = element.else_entry() {
                    self.uncontextual_entry(entry);
                }
            }
            MapElement::For(element) => self.uncontextual_entry(element.entry()),
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
    fn visit_call_arguments(
        &mut self,
        args: ArgList<'a>,
        _positional: &[ResolvedType],
        _named: &HashMap<String, ResolvedType>,
        _state: &SemanticState<'a>,
    ) {
        // The SDK queries correspondingParameter on the closure itself.
        // Unlike named arguments, positional parentheses have no such link.
        for argument in args.positional() {
            if matches!(argument, Expr::Paren(_)) {
                self.uncontextual.insert(argument.syntax().id());
            }
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Paren(paren) => {
                let id = node.syntax().id();
                if self.uncontextual.contains(&id) {
                    self.uncontextual.insert(paren.expr().syntax().id());
                }
                if let Some(function) = self.expression_contexts.get(&id).copied() {
                    self.expression_contexts
                        .insert(paren.expr().syntax().id(), function);
                }
            }
            Expr::Binary(binary) => {
                self.uncontextual
                    .insert(binary.left().unparenthesized().syntax().id());
                self.uncontextual
                    .insert(binary.right().unparenthesized().syntax().id());
            }
            Expr::Await(await_expr) => {
                self.uncontextual.insert(await_expr.expr().syntax().id());
            }
            Expr::Switch(switch) => {
                self.uncontextual
                    .extend(switch.arms().map(|arm| arm.body().syntax().id()));
            }
            Expr::Record(record) => {
                self.uncontextual
                    .extend(record.fields().map(|field| field.value().syntax().id()));
            }
            // The official context approximation does not reach collection
            // literal elements, entries, or the elements under `if`/`for`.
            Expr::List(list) => {
                list.elements()
                    .for_each(|element| self.uncontextual_element(element));
            }
            Expr::Set(set) => {
                set.elements()
                    .for_each(|element| self.uncontextual_element(element));
            }
            Expr::Map(map) => {
                map.elements()
                    .for_each(|element| self.uncontextual_entry(element));
            }
            Expr::Assign(assign) => {
                let function = matches!(
                    state.infer_with_context(node, state.expected()),
                    ResolvedType::Function { .. }
                );
                self.expression_contexts
                    .insert(assign.value().syntax().id(), function);
            }
            Expr::Conditional(conditional) => {
                let function = matches!(
                    state.infer_with_context(node, state.expected()),
                    ResolvedType::Function { .. }
                );
                self.expression_contexts
                    .insert(conditional.then_expr().syntax().id(), function);
                self.expression_contexts
                    .insert(conditional.else_expr().syntax().id(), function);
            }
            _ => {}
        }
        let Expr::FuncExpr(closure) = node else {
            return;
        };
        if closure.async_marker() == AsyncMarker::Async
            && matches!(state.expected(), Some(ResolvedType::Function { return_type, .. })
                if matches!(&**return_type, ResolvedType::Interface { identity, .. }
                    if identity.is_sdk("dart:async", "Future")))
        {
            self.future_closures.insert(node.syntax().id());
        }
        if self.uncontextual.contains(&node.syntax().id())
            || !self.has_function_context(node, state.expected())
        {
            return;
        }
        for param in closure.params().params() {
            // A function-typed formal (`int f()`) is reported whole; otherwise
            // only a named type other than `dynamic` is redundant.
            let span = if param.function_params().is_some() {
                param.span()
            } else {
                match param.param_type() {
                    Some(ty @ DartType::Named(named))
                        if !matches!(
                            state.model.resolve_type_in(ty, &state.type_parameters),
                            ResolvedType::Dynamic
                        ) =>
                    {
                        named.span()
                    }
                    Some(ty @ DartType::Void(_)) => ty.span(),
                    Some(
                        DartType::Named(_)
                        | DartType::Dynamic(_)
                        | DartType::Function(_)
                        | DartType::Record(_),
                    )
                    | None => continue,
                }
            };
            self.diags.push(self.ctx.diagnostic("Remove the unnecessary type annotation from this function expression parameter.", span));
        }
    }
}
