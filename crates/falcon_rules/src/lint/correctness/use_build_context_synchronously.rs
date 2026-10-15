//! Port of Dart SDK 3.13.5's ancestor-relative `AsyncStateVisitor`.
//! Mounted guards compare getter declarations, not receiver objects; function
//! bodies are boundaries, except callbacks of the SDK's protected async APIs.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::{Program, ast::*, visitor::*};

#[path = "use_build_context_semantics.rs"]
mod semantics;

#[derive(Default)]
pub struct UseBuildContextSynchronously;

impl Rule for UseBuildContextSynchronously {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if is_test_file(ctx.file_path) {
            return Vec::new();
        }
        let mut semantics = semantics::collect(program, ctx);
        if semantics.uses.is_empty() {
            return Vec::new();
        }
        let mut tree = Tree {
            never: std::mem::take(&mut semantics.never),
            ..Tree::default()
        };
        tree.visit_program(program);
        let mut diagnostics = Vec::with_capacity(semantics.uses.len());
        let mut visitor = AsyncStateVisitor {
            tree: &tree,
            semantics: &semantics,
            mounted: false,
            reference: Key(0, 0),
            cache: HashMap::new(),
        };
        for (&expression, &mounted) in &semantics.uses {
            let key = Key(expression, 0);
            let Some(node) = tree.nodes.get(&key) else {
                continue;
            };
            visitor.mounted = mounted;
            visitor.reference = key;
            visitor.cache.clear();
            let mut child = key;
            let mut report = false;
            while let Some(parent) = tree.nodes.get(&child).and_then(|n| n.parent) {
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
                    Some(AsyncState::Mounted | AsyncState::NotMounted) => break,
                    Some(AsyncState::Async) => {
                        report = true;
                        break;
                    }
                    None => child = parent,
                }
            }
            if report {
                diagnostics.push(ctx.diagnostic(
                    "use-build-context-synchronously",
                    "Do not use a BuildContext across an async gap without a matching mounted check.",
                    node.span,
                ));
            }
        }
        diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
        diagnostics
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key(usize, u8);
fn expr(node: &Expr) -> Key {
    Key(node as *const Expr as usize, 0)
}
fn stmt(node: &Stmt) -> Key {
    Key(node as *const Stmt as usize, 1)
}
fn block(node: &Block) -> Key {
    Key(node as *const Block as usize, 2)
}
fn collection(node: &CollectionElement) -> Key {
    Key(node as *const CollectionElement as usize, 6)
}
fn map(node: &MapElement) -> Key {
    Key(node as *const MapElement as usize, 7)
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
struct Tree {
    nodes: HashMap<Key, Node>,
    stack: Vec<Key>,
    never: std::collections::HashSet<usize>,
    labels: Vec<(String, Key)>,
    label_breaks: std::collections::HashSet<Key>,
}
impl Tree {
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
    // These syntax containers are explicit ancestors in analyzer, but flattened
    // by our AST. Keep evaluation children unchanged and restore parent identity
    // so the official reference-relative algorithm sees the same barriers.
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
    fn for_exits(&self, header: &ForHeader, body: Key) -> bool {
        if let Some(ForInit::ForIn { iterable, .. } | ForInit::PatternForIn { iterable, .. }) =
            &header.init
        {
            return self.nodes[&expr(iterable)].exits;
        }
        let loop_node = &self.nodes[self.stack.last().unwrap()];
        loop_node
            .children
            .iter()
            .filter(|&&k| k != body)
            .any(|k| self.nodes[k].exits)
            || ((header.condition.is_none()
                || matches!(
                    header.condition.as_ref().map(Expr::unparenthesized),
                    Some(Expr::BoolLit { value: true, .. })
                ))
                && (self.nodes[&body].exits || !self.nodes[&body].breaks))
    }
}
fn condition(node: &IfCondition) -> (Key, Option<Key>) {
    match node {
        IfCondition::Expr(e) => (expr(e), None),
        IfCondition::Case(e, _, guard) => (expr(e), guard.as_deref().map(expr)),
    }
}
impl Visitor for Tree {
    fn visit_expr(&mut self, node: &Expr) {
        let kind = match node {
            Expr::Paren { .. } => Kind::Revisit,
            Expr::Await { .. } => Kind::Await,
            Expr::As { .. } => Kind::Cast,
            Expr::Ident(_) | Expr::Field { .. } => Kind::Mounted,
            Expr::Unary { op, .. } => Kind::Unary(*op == UnaryOp::Bang),
            Expr::Binary { op, .. } => Kind::Binary(*op),
            Expr::Assign { .. } => Kind::Assign,
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => Kind::If {
                condition: expr(condition),
                guard: None,
                yes: expr(then_expr),
                no: Some(expr(else_expr)),
            },
            Expr::FuncExpr { .. } => Kind::None,
            Expr::Throw { .. } => Kind::Exit,
            _ => Kind::Any,
        };
        self.enter(expr(node), kind, *node.span());
        walk_expr(self, node);
        match node {
            Expr::Call { callee, args, .. } => self.group(
                Key(args as *const ArgList as usize, 9),
                expr(node),
                Some(expr(callee)),
            ),
            Expr::New { args, .. } => {
                self.group(Key(args as *const ArgList as usize, 9), expr(node), None)
            }
            _ => {}
        }
        let children = &self.nodes[&expr(node)].children;
        let exits = |key: Key| self.nodes[&key].exits;
        let override_exit = match node {
            Expr::Paren { expr: inner, .. } => {
                let literal = self.nodes[&expr(inner)].literal;
                let exits = self.nodes[&expr(inner)].exits;
                self.nodes.get_mut(&expr(node)).unwrap().literal = literal;
                Some(exits)
            }
            Expr::BoolLit { value, .. } => {
                self.nodes.get_mut(&expr(node)).unwrap().literal = Some(*value);
                Some(false)
            }
            Expr::Unary { .. } | Expr::PostfixIncDec { .. } => Some(false),
            Expr::Binary {
                op, left, right, ..
            } => Some(match op {
                BinaryOp::Or => {
                    exits(expr(left))
                        || (matches!(left.unparenthesized(), Expr::BoolLit { value: false, .. })
                            && exits(expr(right)))
                }
                BinaryOp::And => {
                    exits(expr(left))
                        || (matches!(left.unparenthesized(), Expr::BoolLit { value: true, .. })
                            && exits(expr(right)))
                }
                BinaryOp::NullCoalesce => exits(expr(left)),
                _ => exits(expr(left)) || exits(expr(right)),
            }),
            Expr::Assign {
                target, op, value, ..
            } => Some(
                exits(expr(target))
                    || (*op != AssignOp::NullCoalesceEq
                        && !matches!(
                            target.as_ref(),
                            Expr::Field {
                                is_null_safe: true,
                                ..
                            }
                        )
                        && exits(expr(value))),
            ),
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => Some(exits(expr(condition)) || (exits(expr(then_expr)) && exits(expr(else_expr)))),
            Expr::Call { callee, .. } => Some(
                exits(expr(callee))
                    || (!matches!(
                        callee.as_ref(),
                        Expr::Field {
                            is_null_safe: true,
                            ..
                        }
                    ) && (self.never.contains(&expr(node).0)
                        || children.iter().skip(1).any(|&k| exits(k)))),
            ),
            Expr::Switch { .. } => Some(children.iter().skip(1).all(|&k| exits(k))),
            _ => None,
        };
        self.nodes.get_mut(&expr(node)).unwrap().exit_override = override_exit;
        self.leave();
    }
    fn visit_stmt(&mut self, node: &Stmt) {
        let kind = match node {
            Stmt::Block(_) => Kind::Forward,
            Stmt::Labeled(_) => Kind::Label,
            Stmt::If(n) => {
                let (condition, guard) = condition(&n.condition);
                Kind::If {
                    condition,
                    guard,
                    yes: stmt(&n.then_branch),
                    no: n.else_branch.as_deref().map(stmt),
                }
            }
            Stmt::For(n) => Kind::For {
                condition: n.header.condition.as_ref().map(expr),
                body: stmt(&n.body),
            },
            Stmt::While(_) => Kind::While,
            Stmt::DoWhile(n) => Kind::Do {
                body: stmt(&n.body),
                condition: expr(&n.condition),
            },
            Stmt::TryCatch(n) => Kind::Try {
                body: block(&n.body),
                catches: n
                    .catches
                    .iter()
                    .map(|c| Key(c as *const CatchClause as usize, 5))
                    .collect(),
                finally: n.finally.as_ref().map(block),
            },
            Stmt::Switch(n) => Kind::Switch {
                subject: expr(&n.subject),
                cases: n
                    .cases
                    .iter()
                    .map(|c| Key(c as *const SwitchCase as usize, 4))
                    .collect(),
            },
            Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::Rethrow(_)
            | Stmt::Break(_)
            | Stmt::Continue(_) => Kind::Exit,
            Stmt::Assert(_) | Stmt::LocalFunc(_) => Kind::None,
            Stmt::PatternDecl(_) | Stmt::PatternAssign(_) => Kind::Inert,
            _ => Kind::Any,
        };
        self.enter(stmt(node), kind, *node.span());
        if let Stmt::Labeled(n) = node {
            self.labels.push((n.label.name.clone(), stmt(&n.stmt)));
        }
        if let Stmt::Break(n) = node
            && let Some(label) = &n.label
            && let Some((_, target)) = self
                .labels
                .iter()
                .rev()
                .find(|(name, _)| name == &label.name)
        {
            self.label_breaks.insert(*target);
        }
        walk_stmt(self, node);
        if let Stmt::For(n) = node {
            self.group(
                Key(&n.header as *const ForHeader as usize, 10),
                stmt(node),
                Some(stmt(&n.body)),
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
                let broke = self.label_breaks.remove(&stmt(&n.stmt));
                Some(self.nodes[&stmt(&n.stmt)].exits && !broke)
            }
            Stmt::While(n) => Some(
                self.nodes[&expr(&n.condition)].exits
                    || (matches!(
                        n.condition.unparenthesized(),
                        Expr::BoolLit { value: true, .. }
                    ) && !self.nodes[&stmt(&n.body)].breaks),
            ),
            Stmt::DoWhile(n) => {
                let body = &self.nodes[&stmt(&n.body)];
                Some(
                    (body.exits && !body.breaks && !body.continues)
                        || self.nodes[&expr(&n.condition)].exits
                        || (matches!(
                            n.condition.unparenthesized(),
                            Expr::BoolLit { value: true, .. }
                        ) && !body.breaks),
                )
            }
            Stmt::For(n) => Some(self.for_exits(&n.header, stmt(&n.body))),
            Stmt::Switch(n) => Some(
                n.cases
                    .iter()
                    .any(|c| c.cases.iter().any(|k| matches!(k, SwitchCaseKind::Default)))
                    && n.cases.last().is_some_and(|c| !c.body.is_empty())
                    && n.cases
                        .iter()
                        .filter(|c| !c.body.is_empty())
                        .all(|c| self.nodes[&Key(c as *const SwitchCase as usize, 4)].exits),
            ),
            _ => None,
        };
        self.nodes.get_mut(&stmt(node)).unwrap().exit_override = override_exit;
        self.leave();
    }
    fn visit_block(&mut self, node: &Block) {
        self.enter(block(node), Kind::Block, node.span);
        walk_block(self, node);
        self.leave();
    }
    fn visit_function_body(&mut self, node: &FunctionBody) {
        self.enter(
            Key(node as *const FunctionBody as usize, 3),
            Kind::Body,
            *node.span(),
        );
        walk_function_body(self, node);
        self.leave();
    }
    fn visit_catch_clause(&mut self, node: &CatchClause) {
        self.enter(
            Key(node as *const CatchClause as usize, 5),
            Kind::Catch,
            node.span,
        );
        walk_catch_clause(self, node);
        self.leave();
    }
    fn visit_switch_case(&mut self, node: &SwitchCase) {
        let guards = node
            .cases
            .iter()
            .map(|c| match c {
                SwitchCaseKind::Pattern(_, guard) => Some(guard.as_deref().map(expr)),
                SwitchCaseKind::Expr(_) => Some(None),
                SwitchCaseKind::Default => None,
            })
            .collect();
        self.enter(
            Key(node as *const SwitchCase as usize, 4),
            Kind::Case {
                guards,
                statements: node.body.iter().map(stmt).collect(),
            },
            node.span,
        );
        walk_switch_case(self, node);
        let exits = node.body.iter().any(|s| self.nodes[&stmt(s)].exits);
        self.nodes
            .get_mut(&Key(node as *const SwitchCase as usize, 4))
            .unwrap()
            .exit_override = Some(exits);
        self.leave();
    }
    fn visit_switch_expr_arm(&mut self, node: &SwitchExprArm) {
        self.enter(
            Key(node as *const SwitchExprArm as usize, 8),
            Kind::Arm {
                guard: node.guard.as_ref().map(expr),
                body: expr(&node.body),
            },
            node.span,
        );
        walk_switch_expr_arm(self, node);
        self.leave();
    }
    fn visit_collection_element(&mut self, node: &CollectionElement) {
        let (kind, span) = match node {
            CollectionElement::Expr(e) => (Kind::Any, *e.span()),
            CollectionElement::NullAware { span, .. } | CollectionElement::Spread { span, .. } => {
                (Kind::Any, *span)
            }
            CollectionElement::If {
                condition: c,
                then_elem,
                else_elem,
                span,
            } => {
                let (condition, guard) = condition(c);
                (
                    Kind::If {
                        condition,
                        guard,
                        yes: collection(then_elem),
                        no: else_elem.as_deref().map(collection),
                    },
                    *span,
                )
            }
            CollectionElement::For {
                header,
                element,
                span,
                ..
            } => (
                Kind::For {
                    condition: header.condition.as_ref().map(expr),
                    body: collection(element),
                },
                *span,
            ),
        };
        self.enter(collection(node), kind, span);
        walk_collection_element(self, node);
        if let CollectionElement::For {
            header, element, ..
        } = node
        {
            self.group(
                Key(header as *const ForHeader as usize, 10),
                collection(node),
                Some(collection(element)),
            );
            let exits = self.for_exits(header, collection(element));
            self.nodes.get_mut(&collection(node)).unwrap().exit_override = Some(exits);
        }
        self.leave();
    }
    fn visit_map_element(&mut self, node: &MapElement) {
        let (kind, span) = match node {
            MapElement::Entry(e) => (Kind::Any, e.span),
            MapElement::Spread { span, .. } => (Kind::Any, *span),
            MapElement::If {
                condition: c,
                then_entry,
                else_entry,
                span,
            } => {
                let (condition, guard) = condition(c);
                (
                    Kind::If {
                        condition,
                        guard,
                        yes: map(then_entry),
                        no: else_entry.as_deref().map(map),
                    },
                    *span,
                )
            }
            MapElement::For {
                header,
                entry,
                span,
                ..
            } => (
                Kind::For {
                    condition: header.condition.as_ref().map(expr),
                    body: map(entry),
                },
                *span,
            ),
        };
        self.enter(map(node), kind, span);
        walk_map_element(self, node);
        if let MapElement::For { header, entry, .. } = node {
            self.group(
                Key(header as *const ForHeader as usize, 10),
                map(node),
                Some(map(entry)),
            );
            let exits = self.for_exits(header, map(entry));
            self.nodes.get_mut(&map(node)).unwrap().exit_override = Some(exits);
        }
        self.leave();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AsyncState {
    Async,
    Mounted,
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
struct AsyncStateVisitor<'a> {
    tree: &'a Tree,
    semantics: &'a semantics::Semantics,
    mounted: bool,
    reference: Key,
    cache: HashMap<Key, State>,
}
impl AsyncStateVisitor<'_> {
    fn any(&self, nodes: &[Key]) -> State {
        nodes
            .iter()
            .take_while(|&&k| k != self.reference)
            .any(|&k| self.state(k) == Some(AsyncState::Async))
            .then_some(AsyncState::Async)
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
        match &node.kind {
            Kind::None | Kind::Inert | Kind::Body | Kind::Exit => None,
            Kind::Await => self.cache.get(&key).copied().unwrap_or_else(|| {
                (children.first() != Some(&self.reference)).then_some(AsyncState::Async)
            }),
            Kind::Mounted => self.any(children).or_else(|| {
                self.semantics
                    .mounted
                    .get(&key.0)
                    .filter(|&&m| m == self.mounted)
                    .map(|_| AsyncState::Mounted)
            }),
            Kind::Revisit => children.first().and_then(|&k| self.state(k)),
            Kind::Cast => children.first().and_then(|&k| asynchronous(self.state(k))),
            Kind::Unary(true) => invert(children.first().and_then(|&k| self.state(k))),
            Kind::Unary(false) => None,
            Kind::Binary(op) => self.binary(*op, children),
            Kind::Assign => self.ordered(children, |k| children.last() == Some(&k)),
            // Stmt::Block wraps a Block in our AST; analyzer has one node.
            Kind::Forward => children
                .first()
                .filter(|&&k| k != self.reference)
                .and_then(|&k| self.state(k)),
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
            } => self.if_like(*condition, *guard, *yes, *no),
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
                    (l, self.semantics.constants.get(&right.0))
                } else {
                    (r, self.semantics.constants.get(&left.0))
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

fn is_test_file(path: &std::path::Path) -> bool {
    crate::lint::declarations::in_test_directory(path)
}

#[cfg(test)]
mod test_directories {
    use super::is_test_file;
    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    #[test]
    fn test_directory_exemptions_are_package_root_relative() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("pubspec.yaml"), "name: example\n").unwrap();
        let source = "class Suite { test_example() {} solo_test_example() {} }";
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = FalconConfig::default();
        for (relative, expected) in [
            ("test/suite.dart", true),
            ("integration_test/suite.dart", true),
            ("test_driver/suite.dart", true),
            ("testing/suite.dart", true),
            ("lib/src/test/suite.dart", false),
            ("lib/testing/suite.dart", false),
            ("lib/suite.dart", false),
        ] {
            let path = directory.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, source).unwrap();
            assert_eq!(is_test_file(&path), expected, "{relative}");
            let ctx = AnalyzeContext::new(&path, source, &config);
            let diagnostics =
                crate::lint::style::always_declare_return_types::AlwaysDeclareReturnTypes
                    .analyze(&program, &ctx);
            assert_eq!(
                diagnostics.len(),
                if expected { 0 } else { 2 },
                "{relative}"
            );
        }
    }
}
