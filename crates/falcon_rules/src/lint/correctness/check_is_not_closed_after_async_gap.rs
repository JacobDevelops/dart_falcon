//! Flags Bloc `add` and `emit` calls after an await without a fresh `isClosed` check.
//!
//! A Bloc or Cubit can close while asynchronous work is suspended. Check
//! `!isClosed` before dispatching or emitting, or return when `isClosed` is true.
//! Each await invalidates earlier checks. Both guarded branches and terminating
//! early-return guards are recognized, and nested functions are independent.
//! Awaits in call arguments invalidate even an enclosing guard.
//! Calls must target the enclosing package-backed Bloc/Cubit, or an Emitter
//! parameter of its event handler. An Emitter also accepts its own `!isDone`
//! guard. Unrelated receivers, shadowed names and opaque mixins are ignored.
//! Analysis follows straight-line statements and enclosing if branches;
//! uncertain loops, switch/try flow, pattern conditions, conditional collection
//! elements and optional/short-circuit evaluation are not treated as proof of
//! an unguarded asynchronous path.
//!
//! ### Bad
//! ```dart
//! await load();
//! emit(Loaded());
//! ```
//!
//! ### Good
//! ```dart
//! await load();
//! if (isClosed) return;
//! emit(Loaded());
//! ```

use std::collections::{HashMap, HashSet};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::bloc::own_member;
use crate::lint::bloc::BlocTypes;

#[derive(Default)]
pub struct CheckIsNotClosedAfterAsyncGap;

impl Rule for CheckIsNotClosedAfterAsyncGap {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            types: BlocTypes::new(program),
            calls: Vec::new(),
            awaits: Vec::new(),
            closed: HashSet::new(),
            emitter_calls: HashMap::new(),
            done: HashMap::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    types: BlocTypes<'a>,
    calls: Vec<CallExpr<'a>>,
    awaits: Vec<Expr<'a>>,
    closed: HashSet<NodeId>,
    emitter_calls: HashMap<NodeId, Span>,
    done: HashMap<NodeId, Span>,
}

impl<'a> Collector<'a> {
    fn emitter(&self, callee: Expr<'a>, state: &SemanticState<'a>) -> bool {
        let Expr::Ident(identifier) = callee.unparenthesized() else {
            return false;
        };
        let name = identifier.name();
        if !state.environment.is_bound(name) {
            return false;
        }
        let Some(class) = callee.syntax().ancestors().find_map(ClassDecl::cast) else {
            return false;
        };
        if !self
            .types
            .is_bloc_class(class, &state.model, Some(&state.signatures))
        {
            return false;
        }
        if self
            .types
            .is_api_type(&state.environment.lookup(name), &state.model, "Emitter")
        {
            return true;
        }
        let declaration = state.environment.binding_declaration(name);
        if self
            .types
            .binding_has_api_syntax(callee, state, &["Emitter"])
        {
            return true;
        }
        // `on<Event>((event, emit) ...)` contextually supplies an Emitter even
        // when the external package's declarations are unavailable.
        let Some(function) =
            callee
                .syntax()
                .ancestors()
                .find_map(|ancestor| match Expr::cast(ancestor) {
                    Some(Expr::FuncExpr(function)) => Some(function),
                    _ => None,
                })
        else {
            return false;
        };
        let Some(parameter) = function.params().params().nth(1) else {
            return false;
        };
        Some(parameter.name().span()) == declaration
            && function
                .syntax()
                .ancestors()
                .skip(1)
                .find_map(|ancestor| match Expr::cast(ancestor) {
                    Some(Expr::Call(call)) => Some(call),
                    _ => None,
                })
                .is_some_and(|call| own_member(call.callee(), "on", &self.types, state))
    }

    fn condition_open(&self, expression: Expr<'a>, truth: bool, emitter: Option<Span>) -> bool {
        let mut proof = GuardProof {
            closed: &self.closed,
            done: &self.done,
            emitter,
            values: [[false; 2]; 2],
            count: 0,
        };
        visit::Visitor::visit_expr(&mut proof, expression);
        proof.values[0][usize::from(truth)]
    }

    fn guarded(&self, statement: IfStmt<'a>, truth: bool, emitter: Option<Span>) -> bool {
        matches!(statement.condition(), IfCondition::Expr(expression) if self.condition_open(expression, truth, emitter))
    }

    fn previous(&self, statement: Stmt<'a>, body: NodeId, emitter: Option<Span>) -> Evidence {
        if let Stmt::If(branch) = statement {
            if terminates(branch.then_branch()) && self.guarded(branch, false, emitter)
                || branch.else_branch().is_some_and(terminates)
                    && self.guarded(branch, true, emitter)
            {
                return Evidence::Guard;
            }
            return Evidence::Barrier;
        }
        if !matches!(
            statement,
            Stmt::Expr(_) | Stmt::LocalVar(_) | Stmt::Empty(_)
        ) {
            return Evidence::Barrier;
        }
        let mut result = Evidence::None;
        for awaited in &self.awaits {
            if statement.span().contains(awaited.span())
                && function_body(awaited.syntax()) == Some(body)
            {
                if awaited
                    .syntax()
                    .ancestors()
                    .take_while(|node| *node != statement.syntax())
                    .any(uncertain_expression)
                {
                    return Evidence::Barrier;
                }
                result = Evidence::Gap;
            }
        }
        result
    }

    fn violates(&self, call: CallExpr<'a>) -> bool {
        let Some(body) = function_body(call.syntax()) else {
            return false;
        };
        if self.awaits.iter().any(|awaited| {
            call.args().span().contains(awaited.span())
                && function_body(awaited.syntax()) == Some(body)
                && !awaited
                    .syntax()
                    .ancestors()
                    .take_while(|node| *node != call.syntax())
                    .any(uncertain_expression)
        }) {
            return true;
        }
        let emitter = self.emitter_calls.get(&call.syntax().id()).copied();
        let mut child = call.syntax();
        while let Some(parent) = child.parent() {
            if uncertain_expression(parent) {
                return false;
            }
            if let Some(block) = Block::cast(parent) {
                for statement in block
                    .stmts()
                    .rev()
                    .filter(|statement| statement.span().end <= child.span().start)
                {
                    match self.previous(statement, body, emitter) {
                        Evidence::Gap => return true,
                        Evidence::Guard | Evidence::Barrier => return false,
                        Evidence::None => {}
                    }
                }
            }
            if parent.id() == body {
                return false;
            }
            if let Some(Stmt::If(branch)) = Stmt::cast(parent) {
                let truth = branch.then_branch().syntax() == child;
                if let IfCondition::Case(condition) = branch.condition()
                    && (self.condition_open(condition.expr(), false, emitter)
                        || self.condition_open(condition.expr(), true, emitter)
                        || (truth
                            && condition
                                .guard()
                                .is_some_and(|guard| self.condition_open(guard, true, emitter))))
                {
                    return false;
                }
                if (truth
                    || branch
                        .else_branch()
                        .is_some_and(|statement| statement.syntax() == child))
                    && self.guarded(branch, truth, emitter)
                {
                    return false;
                }
            } else if Stmt::cast(parent).is_some_and(|statement| {
                matches!(
                    statement,
                    Stmt::For(_)
                        | Stmt::While(_)
                        | Stmt::DoWhile(_)
                        | Stmt::TryCatch(_)
                        | Stmt::Switch(_)
                )
            }) {
                return false;
            }
            child = parent;
        }
        false
    }
}

#[derive(Clone, Copy)]
enum Evidence {
    None,
    Gap,
    Guard,
    Barrier,
}


fn function_body(node: SyntaxNode<'_>) -> Option<NodeId> {
    node.ancestors()
        .find(|ancestor| {
            FunctionBody::cast(*ancestor).is_some()
                && ancestor.parent().is_some_and(|parent| {
                    MethodDecl::cast(parent).is_some()
                        || ConstructorDecl::cast(parent).is_some()
                        || FunctionDecl::cast(parent).is_some()
                        || GetterDecl::cast(parent).is_some()
                        || SetterDecl::cast(parent).is_some()
                        || matches!(Expr::cast(parent), Some(Expr::FuncExpr(_)))
                        || matches!(Stmt::cast(parent), Some(Stmt::LocalFunc(_)))
                })
        })
        .map(|body| body.id())
}

fn uncertain_expression(node: SyntaxNode<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::IfElement
            | NodeKind::ForElement
            | NodeKind::MapIfElement
            | NodeKind::MapForElement
    ) || matches!(
        Expr::cast(node),
        Some(Expr::Conditional(_) | Expr::Switch(_))
    ) || matches!(Expr::cast(node), Some(Expr::Binary(binary)) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce))
        || matches!(Expr::cast(node), Some(Expr::Call(call)) if optional_receiver(call.callee()))
        || matches!(Expr::cast(node), Some(Expr::Cascade(cascade)) if cascade.is_null_aware())
}

fn optional_receiver(expression: Expr<'_>) -> bool {
    let mut optional = false;
    visit::for_each_expr(expression, &mut |node| {
        optional |= match node {
            Expr::Field(field) => field.is_null_safe(),
            Expr::Index(index) => index.is_null_safe(),
            Expr::Cascade(cascade) => cascade.is_null_aware(),
            _ => false,
        };
    });
    optional
}

fn terminates(statement: Stmt<'_>) -> bool {
    match statement {
        Stmt::Return(_) | Stmt::Throw(_) | Stmt::Rethrow(_) => true,
        Stmt::Block(block) => block.stmts().last().is_some_and(terminates),
        _ => false,
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.calls
            .iter()
            .filter(|call| self.violates(**call))
            .map(|call| {
                self.ctx.diagnostic("Check `isClosed` after the await before adding an event or emitting a state.", call.span())
            })
            .collect()
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if own_member(node, "isClosed", &self.types, state) {
            self.closed.insert(node.syntax().id());
        }
        if let Expr::Field(field) = node
            && field.field().name() == "isDone"
            && self.emitter(field.object(), state)
            && let Expr::Ident(identifier) = field.object().unparenthesized()
            && let Some(binding) = state.environment.binding_declaration(identifier.name())
        {
            self.done.insert(node.syntax().id(), binding);
        }
        match node {
            Expr::Await(_) => self.awaits.push(node),
            Expr::Call(call) => {
                let emitter = self.emitter(call.callee(), state);
                if !emitter
                    && !own_member(call.callee(), "add", &self.types, state)
                    && !own_member(call.callee(), "emit", &self.types, state)
                {
                    return;
                }
                if emitter
                    && let Expr::Ident(identifier) = call.callee().unparenthesized()
                    && let Some(binding) = state.environment.binding_declaration(identifier.name())
                {
                    self.emitter_calls.insert(call.syntax().id(), binding);
                }
                self.calls.push(call)
            }
            _ => {}
        }
    }
}

/// Computes both truth-direction proofs using the exhaustive expression walker.
/// Child summaries stay on the call stack; boolean guards need at most two.
#[derive(Clone, Copy)]
struct GuardProof<'s> {
    closed: &'s HashSet<NodeId>,
    done: &'s HashMap<NodeId, Span>,
    emitter: Option<Span>,
    values: [[bool; 2]; 2],
    count: usize,
}

impl<'a> visit::Visitor<'a> for GuardProof<'_> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        let mut children = Self {
            values: [[false; 2]; 2],
            count: 0,
            ..*self
        };
        visit::walk_expr(&mut children, node);
        let [left, right] = children.values;
        let value = if self.closed.contains(&node.syntax().id())
            || self
                .emitter
                .is_some_and(|binding| self.done.get(&node.syntax().id()) == Some(&binding))
        {
            [true, false]
        } else {
            match node {
                Expr::Paren(_) => left,
                Expr::Unary(unary) if unary.op() == UnaryOp::Bang => [left[1], left[0]],
                Expr::Binary(binary) if binary.op() == BinaryOp::And => {
                    [false, left[1] || right[1]]
                }
                Expr::Binary(binary) if binary.op() == BinaryOp::Or => [left[0] || right[0], false],
                Expr::Binary(binary) if matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) => {
                    let comparison = match (
                        binary.left().unparenthesized(),
                        binary.right().unparenthesized(),
                    ) {
                        (_, Expr::BoolLit(value)) => Some((left, value.value())),
                        (Expr::BoolLit(value), _) => Some((right, value.value())),
                        _ => None,
                    };
                    comparison.map_or([false; 2], |(subject, value)| {
                        let polarity = value == (binary.op() == BinaryOp::EqEq);
                        [
                            subject[usize::from(!polarity)],
                            subject[usize::from(polarity)],
                        ]
                    })
                }
                _ => [false; 2],
            }
        };
        self.values[self.count.min(1)] = value;
        self.count += 1;
    }
}
