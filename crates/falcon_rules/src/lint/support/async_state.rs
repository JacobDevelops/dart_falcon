//! Ancestor-relative asynchronous gap analysis.
//!
//! API-specific guards share the evaluation tree. Conservative analysis requires
//! an unconditional gap in preceding straight-line statements; ancestor-relative
//! analysis also follows conditional evaluation and control-flow constructs.

use super::async_semantics as semantics;
use falcon_analyze::{AnalyzeContext, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    Visitor, walk_block, walk_catch_clause, walk_collection_element, walk_expr, walk_function_body,
    walk_map_element, walk_stmt, walk_switch_case, walk_switch_expr_arm,
};
use falcon_syntax::cst::*;
use std::collections::{HashMap, HashSet};

/// Guard-independent facts consumed by the shared gap engine.
#[derive(Default)]
pub(crate) struct AsyncGapFacts {
    constants: HashMap<NodeId, bool>,
    never: HashSet<NodeId>,
    protected_callbacks: HashSet<NodeId>,
}
pub(crate) fn prepare_setstate<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
) -> RulePreparation<'a> {
    semantics::prepare_setstate(program, ctx).map_or_else(
        || RulePreparation::Diagnostics(Vec::new()),
        |collector| RulePreparation::Semantic(Box::new(collector)),
    )
}

pub(super) fn analyze_semantics(
    program: Program<'_>,
    ctx: &AnalyzeContext,
    semantics: semantics::Semantics,
) -> Vec<Diagnostic> {
    let message = semantics.message;
    async_gap_spans(program, semantics)
        .into_iter()
        .map(|span| ctx.diagnostic(message, span))
        .collect()
}

pub(crate) fn async_gap_spans(
    program: Program<'_>,
    mut semantics: semantics::Semantics,
) -> Vec<Span> {
    let guards = std::mem::take(&mut semantics.mounted);
    let uses = std::mem::take(&mut semantics.uses);
    let facts = AsyncGapFacts {
        constants: semantics.constants,
        never: semantics.never,
        protected_callbacks: semantics.protected_callbacks,
    };
    async_gap_spans_with_guard(program, facts, uses, false, |expression, kind| {
        guards
            .get(&expression)
            .filter(|&value| value == kind)
            .map(|_| true)
    })
}

/// Shares evaluation order and gap invalidation while allowing each API to
/// identify its own guard and choose conservative handling of uncertain flow.
/// Guard values describe whether the expression being true means safe.
pub(crate) fn async_gap_spans_with_guard<G>(
    program: Program<'_>,
    mut semantics: AsyncGapFacts,
    uses: HashMap<NodeId, G>,
    conservative: bool,
    guard: impl Fn(NodeId, &G) -> Option<bool>,
) -> Vec<Span> {
    let Some(first_use) = uses.values().next() else {
        return Vec::new();
    };
    let mut tree = Tree {
        never: std::mem::take(&mut semantics.never),
        conservative,
        ..Tree::default()
    };
    tree.visit_program(program);
    let mut diagnostics = Vec::with_capacity(uses.len());
    let mut visitor = AsyncStateVisitor {
        tree: &tree,
        semantics: &semantics,
        guard: &guard,
        conservative,
        use_kind: first_use,
        reference: Key(NodeId(0), 0),
        cache: HashMap::new(),
    };
    for (&expression, kind) in &uses {
        let key = Key(expression, 0);
        let Some(node) = tree.nodes.get(&key) else {
            continue;
        };
        visitor.use_kind = kind;
        visitor.reference = key;
        visitor.cache.clear();
        let mut child = key;
        let mut report = conservative && visitor.gaps(key).0;
        while !report && let Some(parent) = tree.nodes.get(&child).and_then(|n| n.parent) {
            if matches!(tree.nodes[&parent].kind, Kind::Body) {
                if let Some(callback) = tree.nodes[&parent].parent {
                    report = semantics.protected_callbacks.contains(&callback.0);
                }
                break;
            }
            visitor.reference = child;
            let state = visitor.state(parent);
            visitor.cache.insert(parent, state);
            match state {
                Some(AsyncState::Mounted | AsyncState::NotMounted | AsyncState::Barrier) => break,
                Some(AsyncState::Async) => {
                    report = true;
                    break;
                }
                None => child = parent,
            }
        }
        if report {
            diagnostics.push(node.span);
        }
    }
    diagnostics.sort_by_key(|span| span.start);
    diagnostics
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key(NodeId, u8);
fn expr(node: Expr<'_>) -> Key {
    Key(node.syntax().id(), 0)
}
fn stmt(node: Stmt<'_>) -> Key {
    Key(node.syntax().id(), 1)
}
fn block(node: Block<'_>) -> Key {
    Key(node.syntax().id(), 2)
}
fn collection(node: CollectionElement<'_>) -> Key {
    Key(node.syntax().id(), 6)
}
fn map(node: MapElement<'_>) -> Key {
    Key(node.syntax().id(), 7)
}

#[derive(Clone)]
enum Kind {
    Any,
    None,
    Inert,
    Forward,
    Revisit,
    Cast,
    Label,
    Body,
    Block,
    Exit,
    Await,
    Mounted,
    Barrier,
    Uncertain,
    Unary(bool),
    Binary(BinaryOp),
    Assign,
    If {
        condition: Key,
        guard: Option<Key>,
        yes: Key,
        no: Option<Key>,
    },
    For {
        condition: Option<Key>,
        body: Key,
    },
    While,
    Do {
        body: Key,
        condition: Key,
    },
    Try {
        body: Key,
        catches: Vec<Key>,
        finally: Option<Key>,
    },
    Catch,
    Switch {
        subject: Key,
        cases: Vec<Key>,
    },
    Case {
        guards: Vec<Option<Option<Key>>>,
        statements: Vec<Key>,
    },
    Arm {
        guard: Option<Key>,
        body: Key,
    },
}
struct Node {
    kind: Kind,
    children: Vec<Key>,
    parent: Option<Key>,
    span: Span,
    terminates: bool,
    exits: bool,
    exit_override: Option<bool>,
    breaks: bool,
    continues: bool,
    literal: Option<bool>,
}
#[derive(Default)]
struct Tree<'a> {
    nodes: HashMap<Key, Node>,
    stack: Vec<Key>,
    never: std::collections::HashSet<NodeId>,
    conservative: bool,
    patterns: std::collections::HashSet<Key>,
    labels: Vec<(&'a str, Key)>,
    label_breaks: std::collections::HashSet<Key>,
}
impl Tree<'_> {
    fn enter(&mut self, key: Key, kind: Kind, span: Span) {
        let parent = self.stack.last().copied();
        if let Some(parent) = parent {
            self.nodes.get_mut(&parent).unwrap().children.push(key);
        }
        self.nodes.insert(
            key,
            Node {
                kind,
                children: Vec::new(),
                parent,
                span,
                terminates: false,
                exits: false,
                exit_override: None,
                breaks: false,
                continues: false,
                literal: None,
            },
        );
        self.stack.push(key);
    }
    // Restore explicit container ancestors after visiting their evaluation
    // children so the reference-relative algorithm sees the same barriers.
    fn group(&mut self, key: Key, parent: Key, skip: Option<Key>) {
        let children: Vec<_> = self.nodes[&parent]
            .children
            .iter()
            .copied()
            .filter(|k| Some(*k) != skip)
            .collect();
        for child in &children {
            self.nodes.get_mut(child).unwrap().parent = Some(key);
        }
        let span = self.nodes[&parent].span;
        self.nodes.insert(
            key,
            Node {
                kind: Kind::Inert,
                children,
                parent: Some(parent),
                span,
                terminates: false,
                exits: false,
                exit_override: None,
                breaks: false,
                continues: false,
                literal: None,
            },
        );
    }
    fn leave(&mut self) {
        let key = self.stack.pop().unwrap();
        let node = &self.nodes[&key];
        let exits = node.exit_override.unwrap_or_else(|| match &node.kind {
            Kind::Exit => true,
            Kind::None | Kind::Body | Kind::Unary(_) => false,
            Kind::If {
                condition, yes, no, ..
            } => {
                self.nodes[condition].exits
                    || match self.nodes[condition].literal {
                        Some(true) => self.nodes[yes].exits,
                        Some(false) => no.is_some_and(|k| self.nodes[&k].exits),
                        None => self.nodes[yes].exits && no.is_some_and(|k| self.nodes[&k].exits),
                    }
            }
            Kind::Try {
                body,
                catches,
                finally,
            } => {
                finally.is_some_and(|k| self.nodes[&k].exits)
                    || (self.nodes[body].exits && catches.iter().all(|k| self.nodes[k].exits))
            }
            _ => node.children.iter().any(|k| self.nodes[k].exits),
        });
        let terminates = match &node.kind {
            Kind::Exit => true,
            Kind::Block => node
                .children
                .last()
                .is_some_and(|k| self.nodes[k].terminates),
            Kind::Forward => node
                .children
                .last()
                .is_some_and(|k| self.nodes[k].terminates),
            _ => exits,
        };
        let isolates_jumps = matches!(
            node.kind,
            Kind::For { .. }
                | Kind::While
                | Kind::Do { .. }
                | Kind::Switch { .. }
                | Kind::None
                | Kind::Body
        );
        let breaks =
            !isolates_jumps && (node.breaks || node.children.iter().any(|k| self.nodes[k].breaks));
        let continues = !isolates_jumps
            && (node.continues || node.children.iter().any(|k| self.nodes[k].continues));
        let node = self.nodes.get_mut(&key).unwrap();
        node.terminates = terminates;
        node.exits = exits;
        node.breaks = breaks;
        node.continues = continues;
    }
    fn for_exits(&self, header: ForHeader<'_>, body: Key) -> bool {
        match header.init() {
            Some(ForInit::ForIn(init)) => return self.nodes[&expr(init.iterable())].exits,
            Some(ForInit::PatternForIn(init)) => return self.nodes[&expr(init.iterable())].exits,
            _ => {}
        }
        let loop_node = &self.nodes[self.stack.last().unwrap()];
        loop_node
            .children
            .iter()
            .filter(|&&k| k != body)
            .any(|k| self.nodes[k].exits)
            || ((header.condition().is_none()
                || matches!(
                    header.condition().map(Expr::unparenthesized),
                    Some(Expr::BoolLit(value)) if value.value()
                ))
                && (self.nodes[&body].exits || !self.nodes[&body].breaks))
    }
}
fn condition(node: IfCondition<'_>) -> (Key, Option<Key>) {
    match node {
        IfCondition::Expr(e) => (expr(e), None),
        IfCondition::Case(c) => (expr(c.expr()), c.guard().map(expr)),
    }
}
fn conservative_expression(node: Expr<'_>) -> bool {
    matches!(node, Expr::Conditional(_) | Expr::Switch(_))
        || matches!(node, Expr::Call(call) if optional_receiver(call.callee()))
        || matches!(node, Expr::Cascade(cascade) if cascade.is_null_aware())
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

impl<'a> Visitor<'a> for Tree<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        if self.conservative && conservative_expression(node) {
            self.enter(expr(node), Kind::Uncertain, node.span());
            walk_expr(self, node);
            self.leave();
            return;
        }
        let kind = match node {
            Expr::Paren(_) => Kind::Revisit,
            Expr::Await(_) => Kind::Await,
            Expr::As(_) => Kind::Cast,
            Expr::Ident(_) | Expr::Field(_) => Kind::Mounted,
            Expr::Unary(n) => Kind::Unary(n.op() == UnaryOp::Bang),
            Expr::Binary(n) => Kind::Binary(n.op()),
            Expr::Assign(_) => Kind::Assign,
            Expr::Conditional(n) => Kind::If {
                condition: expr(n.condition()),
                guard: None,
                yes: expr(n.then_expr()),
                no: Some(expr(n.else_expr())),
            },
            Expr::FuncExpr(_) => Kind::None,
            Expr::Throw(_) => Kind::Exit,
            _ => Kind::Any,
        };
        self.enter(expr(node), kind, node.span());
        walk_expr(self, node);
        match node {
            Expr::Call(n) => self.group(
                Key(n.args().syntax().id(), 9),
                expr(node),
                Some(expr(n.callee())),
            ),
            Expr::New(n) => self.group(Key(n.args().syntax().id(), 9), expr(node), None),
            _ => {}
        }
        let children = &self.nodes[&expr(node)].children;
        let exits = |key: Key| self.nodes[&key].exits;
        let override_exit = match node {
            Expr::Paren(n) => {
                let literal = self.nodes[&expr(n.expr())].literal;
                let exits = self.nodes[&expr(n.expr())].exits;
                self.nodes.get_mut(&expr(node)).unwrap().literal = literal;
                Some(exits)
            }
            Expr::BoolLit(n) => {
                self.nodes.get_mut(&expr(node)).unwrap().literal = Some(n.value());
                Some(false)
            }
            Expr::Unary(_) | Expr::PostfixIncDec(_) => Some(false),
            Expr::Binary(n) => Some(match n.op() {
                BinaryOp::Or => {
                    exits(expr(n.left()))
                        || (matches!(
                            n.left().unparenthesized(),
                            Expr::BoolLit(value) if !value.value()
                        ) && exits(expr(n.right())))
                }
                BinaryOp::And => {
                    exits(expr(n.left()))
                        || (matches!(
                            n.left().unparenthesized(),
                            Expr::BoolLit(value) if value.value()
                        ) && exits(expr(n.right())))
                }
                BinaryOp::NullCoalesce => exits(expr(n.left())),
                _ => exits(expr(n.left())) || exits(expr(n.right())),
            }),
            Expr::Assign(n) => Some(
                exits(expr(n.target()))
                    || (n.op() != AssignOp::NullCoalesceEq
                        && !matches!(n.target(), Expr::Field(field) if field.is_null_safe())
                        && exits(expr(n.value()))),
            ),
            Expr::Conditional(n) => Some(
                exits(expr(n.condition()))
                    || (exits(expr(n.then_expr())) && exits(expr(n.else_expr()))),
            ),
            Expr::Call(n) => Some(
                exits(expr(n.callee()))
                    || (!matches!(n.callee(), Expr::Field(field) if field.is_null_safe())
                        && (self.never.contains(&expr(node).0)
                            || children.iter().skip(1).any(|&k| exits(k)))),
            ),
            Expr::Switch(_) => Some(children.iter().skip(1).all(|&k| exits(k))),
            _ => None,
        };
        self.nodes.get_mut(&expr(node)).unwrap().exit_override = override_exit;
        self.leave();
    }
    fn visit_stmt(&mut self, node: Stmt<'a>) {
        if self.conservative
            && matches!(node, Stmt::If(branch) if matches!(branch.condition(), IfCondition::Case(_)))
        {
            self.patterns.insert(stmt(node));
        }
        let kind = match node {
            Stmt::Block(_) => Kind::Forward,
            Stmt::Return(_) | Stmt::Throw(_) | Stmt::Rethrow(_) => Kind::Exit,
            _ if self.conservative
                && !matches!(
                    node,
                    Stmt::If(_)
                        | Stmt::Expr(_)
                        | Stmt::LocalVar(_)
                        | Stmt::Empty(_)
                        | Stmt::Labeled(_)
                ) =>
            {
                Kind::Barrier
            }
            Stmt::Labeled(_) => Kind::Label,
            Stmt::If(n) => {
                let (condition, guard) = condition(n.condition());
                Kind::If {
                    condition,
                    guard,
                    yes: stmt(n.then_branch()),
                    no: n.else_branch().map(stmt),
                }
            }
            Stmt::For(n) => Kind::For {
                condition: n.header().condition().map(expr),
                body: stmt(n.body()),
            },
            Stmt::While(_) => Kind::While,
            Stmt::DoWhile(n) => Kind::Do {
                body: stmt(n.body()),
                condition: expr(n.condition()),
            },
            Stmt::TryCatch(n) => Kind::Try {
                body: block(n.body()),
                catches: n.catches().map(|c| Key(c.syntax().id(), 5)).collect(),
                finally: n.finally().map(block),
            },
            Stmt::Switch(n) => Kind::Switch {
                subject: expr(n.subject()),
                cases: n.cases().map(|c| Key(c.syntax().id(), 4)).collect(),
            },
            Stmt::Break(_) | Stmt::Continue(_) => Kind::Exit,
            Stmt::Assert(_) | Stmt::LocalFunc(_) => Kind::None,
            Stmt::PatternDecl(_) | Stmt::PatternAssign(_) => Kind::Inert,
            _ => Kind::Any,
        };
        self.enter(stmt(node), kind, node.span());
        if let Stmt::Labeled(n) = node {
            self.labels.push((n.label().name(), stmt(n.stmt())));
        }
        if let Stmt::Break(n) = node
            && let Some(label) = n.label()
            && let Some((_, target)) = self
                .labels
                .iter()
                .rev()
                .find(|(name, _)| *name == label.name())
        {
            self.label_breaks.insert(*target);
        }
        walk_stmt(self, node);
        if let Stmt::For(n) = node {
            self.group(
                Key(n.header().syntax().id(), 10),
                stmt(node),
                Some(stmt(n.body())),
            );
        }
        let override_exit = match node {
            Stmt::Break(_) => {
                self.nodes.get_mut(&stmt(node)).unwrap().breaks = true;
                Some(false)
            }
            Stmt::Continue(_) => {
                self.nodes.get_mut(&stmt(node)).unwrap().continues = true;
                Some(false)
            }
            Stmt::Labeled(n) => {
                self.labels.pop();
                let broke = self.label_breaks.remove(&stmt(n.stmt()));
                Some(self.nodes[&stmt(n.stmt())].exits && !broke)
            }
            Stmt::While(n) => Some(
                self.nodes[&expr(n.condition())].exits
                    || (matches!(
                        n.condition().unparenthesized(),
                        Expr::BoolLit(value) if value.value()
                    ) && !self.nodes[&stmt(n.body())].breaks),
            ),
            Stmt::DoWhile(n) => {
                let body = &self.nodes[&stmt(n.body())];
                Some(
                    (body.exits && !body.breaks && !body.continues)
                        || self.nodes[&expr(n.condition())].exits
                        || (matches!(
                            n.condition().unparenthesized(),
                            Expr::BoolLit(value) if value.value()
                        ) && !body.breaks),
                )
            }
            Stmt::For(n) => Some(self.for_exits(n.header(), stmt(n.body()))),
            Stmt::Switch(n) => Some(
                n.cases()
                    .any(|c| c.cases().any(|k| matches!(k, SwitchCaseKind::Default(_))))
                    && n.cases().last().is_some_and(|c| !c.body().is_empty())
                    && n.cases()
                        .filter(|c| !c.body().is_empty())
                        .all(|c| self.nodes[&Key(c.syntax().id(), 4)].exits),
            ),
            _ => None,
        };
        self.nodes.get_mut(&stmt(node)).unwrap().exit_override = override_exit;
        self.leave();
    }
    fn visit_block(&mut self, node: Block<'a>) {
        self.enter(block(node), Kind::Block, node.span());
        walk_block(self, node);
        self.leave();
    }
    fn visit_function_body(&mut self, node: FunctionBody<'a>) {
        self.enter(Key(node.syntax().id(), 3), Kind::Body, node.span());
        walk_function_body(self, node);
        self.leave();
    }
    fn visit_catch_clause(&mut self, node: CatchClause<'a>) {
        self.enter(Key(node.syntax().id(), 5), Kind::Catch, node.span());
        walk_catch_clause(self, node);
        self.leave();
    }
    fn visit_switch_case(&mut self, node: SwitchCase<'a>) {
        let guards = node
            .cases()
            .map(|c| match c {
                SwitchCaseKind::Pattern(label) => Some(label.guard().map(expr)),
                SwitchCaseKind::Expr(_) => Some(None),
                SwitchCaseKind::Default(_) => None,
            })
            .collect();
        self.enter(
            Key(node.syntax().id(), 4),
            Kind::Case {
                guards,
                statements: node.body().map(stmt).collect(),
            },
            node.span(),
        );
        walk_switch_case(self, node);
        let exits = node.body().any(|s| self.nodes[&stmt(s)].exits);
        self.nodes
            .get_mut(&Key(node.syntax().id(), 4))
            .unwrap()
            .exit_override = Some(exits);
        self.leave();
    }
    fn visit_switch_expr_arm(&mut self, node: SwitchExprArm<'a>) {
        self.enter(
            Key(node.syntax().id(), 8),
            Kind::Arm {
                guard: node.guard().map(expr),
                body: expr(node.body()),
            },
            node.span(),
        );
        walk_switch_expr_arm(self, node);
        self.leave();
    }
    fn visit_collection_element(&mut self, node: CollectionElement<'a>) {
        let kind = match node {
            CollectionElement::If(_) | CollectionElement::For(_) if self.conservative => {
                Kind::Uncertain
            }
            CollectionElement::Expr(_)
            | CollectionElement::NullAware(_)
            | CollectionElement::Spread(_) => Kind::Any,
            CollectionElement::If(n) => {
                let (condition, guard) = condition(n.condition());
                Kind::If {
                    condition,
                    guard,
                    yes: collection(n.then_elem()),
                    no: n.else_elem().map(collection),
                }
            }
            CollectionElement::For(n) => Kind::For {
                condition: n.header().condition().map(expr),
                body: collection(n.element()),
            },
        };
        self.enter(collection(node), kind, node.span());
        walk_collection_element(self, node);
        if let CollectionElement::For(n) = node {
            self.group(
                Key(n.header().syntax().id(), 10),
                collection(node),
                Some(collection(n.element())),
            );
            let exits = self.for_exits(n.header(), collection(n.element()));
            self.nodes.get_mut(&collection(node)).unwrap().exit_override = Some(exits);
        }
        self.leave();
    }
    fn visit_map_element(&mut self, node: MapElement<'a>) {
        let kind = match node {
            MapElement::If(_) | MapElement::For(_) if self.conservative => Kind::Uncertain,
            MapElement::Entry(_) | MapElement::Spread(_) => Kind::Any,
            MapElement::If(n) => {
                let (condition, guard) = condition(n.condition());
                Kind::If {
                    condition,
                    guard,
                    yes: map(n.then_entry()),
                    no: n.else_entry().map(map),
                }
            }
            MapElement::For(n) => Kind::For {
                condition: n.header().condition().map(expr),
                body: map(n.entry()),
            },
        };
        self.enter(map(node), kind, node.span());
        walk_map_element(self, node);
        if let MapElement::For(n) = node {
            self.group(
                Key(n.header().syntax().id(), 10),
                map(node),
                Some(map(n.entry())),
            );
            let exits = self.for_exits(n.header(), map(n.entry()));
            self.nodes.get_mut(&map(node)).unwrap().exit_override = Some(exits);
        }
        self.leave();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AsyncState {
    Async,
    Mounted,
    Barrier,
    NotMounted,
}
type State = Option<AsyncState>;
fn asynchronous(state: State) -> State {
    state.filter(|s| *s == AsyncState::Async)
}
fn invert(state: State) -> State {
    state.map(|s| match s {
        AsyncState::Mounted => AsyncState::NotMounted,
        AsyncState::NotMounted => AsyncState::Mounted,
        s => s,
    })
}
fn and(left: State, right: State) -> State {
    match (left, right) {
        (None, _) => right,
        (_, None) => left,
        (_, Some(AsyncState::Async)) => right,
        (Some(AsyncState::Async), _) => right,
        _ => left,
    }
}
struct AsyncStateVisitor<'a, G> {
    tree: &'a Tree<'a>,
    semantics: &'a AsyncGapFacts,
    guard: &'a dyn Fn(NodeId, &G) -> Option<bool>,
    conservative: bool,
    use_kind: &'a G,
    reference: Key,
    cache: HashMap<Key, State>,
}
impl<G> AsyncStateVisitor<'_, G> {
    /// Tracks unconditional and optional awaits independently: an immediate
    /// argument gap remains definite even if another argument is conditional.
    fn gaps(&self, key: Key) -> (bool, bool) {
        let node = &self.tree.nodes[&key];
        if matches!(node.kind, Kind::None | Kind::Body) {
            return (false, false);
        }
        let mut gaps = (matches!(node.kind, Kind::Await), false);
        for &child in &node.children {
            let nested = self.gaps(child);
            gaps.0 |= nested.0;
            gaps.1 |= nested.1;
        }
        if matches!(
            node.kind,
            Kind::Uncertain | Kind::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce)
        ) {
            (false, gaps.0 || gaps.1)
        } else {
            gaps
        }
    }

    /// Each entry proves safety when the condition evaluates to false or true.
    /// A conjunction's false outcome and a disjunction's true outcome do not
    /// establish which operand decided the result.
    fn proof(&self, key: Key) -> [bool; 2] {
        let node = &self.tree.nodes[&key];
        let children = &node.children;
        match &node.kind {
            Kind::Mounted => {
                (self.guard)(key.0, self.use_kind).map_or([false; 2], |safe| [!safe, safe])
            }
            Kind::Revisit => children.first().map_or([false; 2], |&key| self.proof(key)),
            Kind::Unary(true) => {
                let [no, yes] = children.first().map_or([false; 2], |&key| self.proof(key));
                [yes, no]
            }
            Kind::Binary(op) => {
                let [left, right] = children.as_slice() else {
                    return [false; 2];
                };
                let left_proof = self.proof(*left);
                let right_proof = self.proof(*right);
                match op {
                    BinaryOp::And => [false, left_proof[1] || right_proof[1]],
                    BinaryOp::Or => [left_proof[0] || right_proof[0], false],
                    BinaryOp::EqEq | BinaryOp::NotEq => {
                        let (proof, value) = if let Some(value) = self.constant(*right) {
                            (left_proof, *value)
                        } else if let Some(value) = self.constant(*left) {
                            (right_proof, *value)
                        } else {
                            return [false; 2];
                        };
                        if value == (*op == BinaryOp::EqEq) {
                            proof
                        } else {
                            [proof[1], proof[0]]
                        }
                    }
                    _ => [false; 2],
                }
            }
            _ => [false; 2],
        }
    }

    fn conservative_if(
        &self,
        key: Key,
        condition: Key,
        guard: Option<Key>,
        yes: Key,
        no: Option<Key>,
    ) -> State {
        let proof = self.proof(condition);
        let pattern = self.tree.patterns.contains(&key);
        let guarded_pattern =
            self.reference == yes && guard.is_some_and(|guard| self.proof(guard)[1]);
        if (pattern && proof.iter().any(|safe| *safe)) || guarded_pattern {
            return Some(AsyncState::Barrier);
        }
        if self.reference == yes {
            return (!pattern && proof[1]).then_some(AsyncState::Mounted);
        }
        if no == Some(self.reference) {
            return (!pattern && proof[0]).then_some(AsyncState::Mounted);
        }
        if self.reference == condition || guard == Some(self.reference) {
            return None;
        }
        Some(AsyncState::Barrier)
    }

    fn any(&self, nodes: &[Key]) -> State {
        let mut result = None;
        for &key in nodes.iter().take_while(|&&key| key != self.reference) {
            match self.state(key) {
                Some(AsyncState::Barrier) => return Some(AsyncState::Barrier),
                Some(AsyncState::Async) if !self.conservative => return Some(AsyncState::Async),
                Some(AsyncState::Async) => result = Some(AsyncState::Async),
                _ => {}
            }
        }
        result
    }
    fn ordered(&self, nodes: &[Key], guard: impl Fn(Key) -> bool) -> State {
        let end = nodes
            .iter()
            .position(|k| *k == self.reference)
            .unwrap_or(nodes.len());
        for &node in nodes[..end].iter().rev() {
            let state = self.state(node);
            if state == Some(AsyncState::Async) || (guard(node) && state.is_some()) {
                return state;
            }
        }
        None
    }
    fn block(&self, nodes: &[Key], loop_body: bool) -> State {
        let state = self.ordered(nodes, |_| true);
        if state.is_some() {
            return state;
        }
        if loop_body && let Some(index) = nodes.iter().position(|k| *k == self.reference) {
            return asynchronous(self.ordered(&nodes[index + 1..], |_| true));
        }
        None
    }
    fn state(&self, key: Key) -> State {
        let node = &self.tree.nodes[&key];
        let children = &node.children;
        if self.conservative {
            if key.1 == 0 {
                if self
                    .tree
                    .nodes
                    .get(&self.reference)
                    .and_then(|node| node.parent)
                    == Some(key)
                {
                    return matches!(node.kind, Kind::Uncertain)
                        .then_some(AsyncState::Barrier)
                        .or_else(|| {
                            matches!(
                                node.kind,
                                Kind::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce)
                            )
                            .then_some(AsyncState::Barrier)
                        });
                }
                return match self.gaps(key) {
                    (_, true) => Some(AsyncState::Barrier),
                    (true, false) => Some(AsyncState::Async),
                    _ => None,
                };
            }
            if matches!(
                node.kind,
                Kind::Catch | Kind::Case { .. } | Kind::Arm { .. }
            ) {
                return Some(AsyncState::Barrier);
            }
        }
        match &node.kind {
            Kind::None | Kind::Inert | Kind::Body | Kind::Exit => None,
            Kind::Barrier => Some(AsyncState::Barrier),
            Kind::Uncertain => {
                if children.contains(&self.reference) || self.any(children).is_some() {
                    Some(AsyncState::Barrier)
                } else {
                    None
                }
            }
            Kind::Await => self.cache.get(&key).copied().unwrap_or_else(|| {
                (children.first() != Some(&self.reference)).then_some(AsyncState::Async)
            }),
            Kind::Mounted => self.any(children).or_else(|| {
                (self.guard)(key.0, self.use_kind).map(|safe| {
                    if safe {
                        AsyncState::Mounted
                    } else {
                        AsyncState::NotMounted
                    }
                })
            }),
            Kind::Revisit => children.first().and_then(|&k| self.state(k)),
            Kind::Cast => children.first().and_then(|&k| asynchronous(self.state(k))),
            Kind::Unary(true) => invert(children.first().and_then(|&k| self.state(k))),
            Kind::Unary(false) => None,
            Kind::Binary(op) => self.binary(*op, children),
            Kind::Assign => self.ordered(children, |k| children.last() == Some(&k)),
            // Statement and block evaluation layers share one analyzer node.
            Kind::Forward if self.conservative && !children.contains(&self.reference) => {
                Some(AsyncState::Barrier)
            }
            Kind::Forward => children
                .first()
                .filter(|&&k| k != self.reference)
                .and_then(|&k| self.state(k)),
            Kind::Label if self.conservative => {
                (!children.contains(&self.reference)).then_some(AsyncState::Barrier)
            }
            Kind::Label => children.first().and_then(|&k| self.state(k)),
            Kind::Block => {
                let parent = node.parent.and_then(|k| self.tree.nodes.get(&k));
                let loop_body = parent.is_some_and(|p| {
                    matches!(p.kind, Kind::Forward)
                        && p.parent.is_some_and(|k| {
                            matches!(
                                self.tree.nodes[&k].kind,
                                Kind::For { .. } | Kind::While | Kind::Do { .. }
                            )
                        })
                });
                self.block(children, loop_body)
            }
            Kind::If {
                condition,
                guard,
                yes,
                no,
            } => {
                if self.conservative {
                    return self.conservative_if(key, *condition, *guard, *yes, *no);
                }
                self.if_like(*condition, *guard, *yes, *no)
            }
            Kind::For { condition, body } => self.ordered(children, |k| {
                self.reference == *body && Some(k) == *condition
            }),
            // Unlike a for-condition, a while-condition cannot guard the body.
            // Inspect both children even when one is the current reference.
            Kind::While => children.iter().find_map(|&k| asynchronous(self.state(k))),
            Kind::Do { body, condition } => {
                if self.reference == *body {
                    asynchronous(self.state(*condition))
                } else if self.reference == *condition {
                    asynchronous(self.state(*body))
                } else {
                    asynchronous(self.state(*condition)).or_else(|| asynchronous(self.state(*body)))
                }
            }
            Kind::Try {
                body,
                catches,
                finally,
            } => {
                if self.reference == *body {
                    None
                } else if catches.contains(&self.reference) {
                    asynchronous(self.state(*body))
                } else if *finally == Some(self.reference) {
                    asynchronous(self.state(*body)).or_else(|| self.any(catches))
                } else {
                    finally
                        .and_then(|k| self.state(k))
                        .or_else(|| asynchronous(self.state(*body)))
                        .or_else(|| self.any(catches))
                }
            }
            Kind::Catch => children.first().and_then(|&k| asynchronous(self.state(k))),
            Kind::Switch { subject, cases } => self.switch(*subject, cases),
            Kind::Case { guards, statements } => {
                // Default scans preceding statements without a pattern guard.
                // The surrounding switch still skips default guard checks.
                if guards.last().is_some_and(Option::is_none) {
                    return self.ordered(statements, |_| true);
                }
                if guards.iter().flatten().any(|g| *g == Some(self.reference)) {
                    return None;
                }
                self.block(statements, false).or_else(|| {
                    if statements.contains(&self.reference) {
                        None
                    } else {
                        guards
                            .iter()
                            .flatten()
                            .flatten()
                            .find_map(|&k| asynchronous(self.state(k)))
                    }
                })
            }
            Kind::Arm { guard, body } => {
                if *guard == Some(self.reference) {
                    None
                } else if *body == self.reference {
                    guard
                        .and_then(|k| self.state(k))
                        .filter(|s| *s != AsyncState::NotMounted)
                } else {
                    guard
                        .and_then(|k| asynchronous(self.state(k)))
                        .or_else(|| asynchronous(self.state(*body)))
                }
            }
            Kind::Any => self.any(children),
        }
    }
    fn constant(&self, mut key: Key) -> Option<&bool> {
        while matches!(self.tree.nodes[&key].kind, Kind::Revisit) {
            key = *self.tree.nodes[&key].children.first()?;
        }
        self.semantics.constants.get(&key.0).or_else(|| {
            if self.conservative {
                self.tree.nodes[&key].literal.as_ref()
            } else {
                None
            }
        })
    }

    fn binary(&self, op: BinaryOp, children: &[Key]) -> State {
        let [left, right] = children else {
            return self.any(children);
        };
        if self.reference == *left {
            return None;
        }
        let l = self.state(*left);
        if self.reference == *right {
            return l.filter(|s| {
                *s == AsyncState::Async
                    || (*s == AsyncState::Mounted && op == BinaryOp::And)
                    || (*s == AsyncState::NotMounted && op == BinaryOp::Or)
            });
        }
        let r = self.state(*right);
        match op {
            BinaryOp::And => and(l, r),
            BinaryOp::Or => match (l, r) {
                (_, Some(AsyncState::Async | AsyncState::NotMounted)) => r,
                (Some(AsyncState::Async | AsyncState::NotMounted), _) => l,
                (Some(AsyncState::Mounted), Some(AsyncState::Mounted)) => l,
                _ => None,
            },
            BinaryOp::EqEq | BinaryOp::NotEq => {
                if l == Some(AsyncState::Async) || r == Some(AsyncState::Async) {
                    return Some(AsyncState::Async);
                }
                let (state, constant) = if l.is_some() {
                    (l, self.constant(*right))
                } else {
                    (r, self.constant(*left))
                };
                constant.and_then(|&value| {
                    if value == (op == BinaryOp::EqEq) {
                        state
                    } else {
                        invert(state)
                    }
                })
            }
            _ => asynchronous(l).or_else(|| asynchronous(r)),
        }
    }
    fn if_like(&self, condition: Key, guard: Option<Key>, yes: Key, no: Option<Key>) -> State {
        if self.reference == condition {
            return None;
        }
        let c = self.state(condition);
        if guard == Some(self.reference) {
            return c.filter(|s| *s != AsyncState::NotMounted);
        }
        let c = and(c, guard.and_then(|k| self.state(k)));
        if self.reference == yes {
            return c.filter(|s| *s != AsyncState::NotMounted);
        }
        if no == Some(self.reference) {
            return match c {
                Some(AsyncState::NotMounted) => Some(AsyncState::Mounted),
                Some(AsyncState::Async) => c,
                _ => None,
            };
        }
        let t = self.state(yes);
        let e = no.and_then(|k| self.state(k));
        let tx = self.tree.nodes[&yes].terminates;
        let ex = no.is_some_and(|k| self.tree.nodes[&k].terminates);
        if (t == Some(AsyncState::NotMounted) && (e == Some(AsyncState::NotMounted) || ex))
            || (e == Some(AsyncState::NotMounted) && tx)
        {
            return Some(AsyncState::NotMounted);
        }
        if (t == Some(AsyncState::Async) && !tx)
            || (e == Some(AsyncState::Async) && !ex)
            || c == Some(AsyncState::Async)
        {
            return Some(AsyncState::Async);
        }
        if (c == Some(AsyncState::Mounted) && ex) || (c == Some(AsyncState::NotMounted) && tx) {
            return Some(AsyncState::NotMounted);
        }
        None
    }
    fn switch(&self, subject: Key, cases: &[Key]) -> State {
        let Some(index) = cases.iter().position(|k| *k == self.reference) else {
            return asynchronous(self.state(subject)).or_else(|| self.any(cases));
        };
        let mut fallthrough = true;
        let mut all_mounted = true;
        for (i, &key) in cases[..=index].iter().enumerate().rev() {
            let Kind::Case { guards, statements } = &self.tree.nodes[&key].kind else {
                continue;
            };
            for guard in guards.iter().rev().flatten() {
                let state = guard.and_then(|k| self.state(k));
                if state == Some(AsyncState::Async) {
                    return state;
                }
                if fallthrough {
                    let falls = i == index || statements.is_empty();
                    if falls {
                        all_mounted &= state == Some(AsyncState::Mounted);
                    } else if all_mounted {
                        return Some(AsyncState::Mounted);
                    }
                    fallthrough &= falls;
                }
            }
        }
        (fallthrough && all_mounted).then_some(AsyncState::Mounted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    fn conservative_reports(body: &str) -> Vec<String> {
        let source = format!("Future<void> f() async {{ {body} }}");
        let parsed = parse(&source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let program = parsed.tree.program(&source);
        #[derive(Default)]
        struct Calls {
            uses: HashMap<NodeId, ()>,
            guards: HashSet<NodeId>,
        }
        impl<'a> Visitor<'a> for Calls {
            fn visit_expr(&mut self, expression: Expr<'a>) {
                if matches!(expression, Expr::Call(call) if matches!(call.callee(), Expr::Ident(name) if name.name() == "emit"))
                {
                    self.uses.insert(expression.syntax().id(), ());
                }
                if matches!(expression, Expr::Ident(name) if name.name() == "isClosed")
                    || matches!(expression, Expr::Field(field) if field.field().name() == "isDone")
                {
                    self.guards.insert(expression.syntax().id());
                }
                walk_expr(self, expression);
            }
        }
        let mut calls = Calls::default();
        calls.visit_program(program);
        async_gap_spans_with_guard(
            program,
            AsyncGapFacts::default(),
            calls.uses,
            true,
            |expression, ()| calls.guards.contains(&expression).then_some(false),
        )
        .into_iter()
        .map(|span| source[span.start..span.end].to_owned())
        .collect()
    }

    #[test]
    fn conservative_guards_prove_only_the_deciding_boolean_outcome() {
        for guard in ["isClosed", "emitter.isDone"] {
            for condition in [
                format!("!{guard}"),
                format!("{guard} == false && ready"),
                format!("!{guard} || ready"),
            ] {
                let expected = if condition.contains("||") { 1 } else { 0 };
                let body = format!("await load(); if ({condition}) emit(1);");
                assert_eq!(conservative_reports(&body).len(), expected, "{body}");
            }
            for branch in [
                format!("if ({guard} && ready) {{ log(); }} else {{ emit(1); }}"),
                format!("if (!({guard} && ready)) emit(1);"),
            ] {
                let body = format!("await load(); {branch}");
                assert_eq!(conservative_reports(&body), ["emit(1)"], "{body}");
            }
        }
    }

    #[test]
    fn conservative_pattern_guards_prove_only_the_then_branch_on_true() {
        for body in [
            "final result = await load(); if (result case Ok() when isClosed) emit(1);",
            "await load(); if (result case Ok() when !isClosed) {} else { emit(1); }",
            "await load(); if (result case Ok() when ready) {} else { emit(1); }",
        ] {
            assert_eq!(conservative_reports(body), ["emit(1)"], "{body}");
        }
    }

    #[test]
    fn conservative_gaps_preserve_straight_line_scanner_boundaries() {
        for body in [
            "try { save(); } catch (error) { emit(1); await log(error); }",
            "foo(emit(1), await load());",
            "foo(await load(), emit(1));",
            "if (await ready()) emit(1);",
            "switch (value) { case 0: await load(); emit(1); }",
            "final value = switch (input) { _ when await ready() => emit(1), _ => 0 };",
            "emit(flag ? await load() : 0);",
            "final value = flag ? await load() : 0; emit(1);",
            "{ await load(); } emit(1);",
            "if (flag) { await load(); } emit(1);",
            "while (flag) { await load(); } emit(1);",
        ] {
            assert!(conservative_reports(body).is_empty(), "{body}");
        }
    }

    #[test]
    fn conservative_gaps_keep_unconditional_argument_and_unary_awaits() {
        for body in [
            "emit(State(await load(), flag ? await load() : null));",
            "emit(State(flag ? await load() : null, await load()));",
            "final delta = -await load(); emit(1);",
            "final delta = ~await load(); emit(1);",
            "await load(); label: { emit(1); }",
        ] {
            assert_eq!(conservative_reports(body).len(), 1, "{body}");
        }
    }
}
