//! Flags consecutive statements that invoke members on the same variable.
//!
//! ```dart
//! final list = <int>[];   // bad
//! list.add(1);
//! list.add(2);
//!
//! final list = <int>[]    // good
//!   ..add(1)
//!   ..add(2);
//! ```
//!
//! Within one block (or `switch` case body) the rule follows the official
//! lint's cascade model: a statement that calls a method on, reads a property
//! of, sets a property of, or cascades on a simple variable `v` can join the
//! preceding statement when that one declared `v` (not as `const`), assigned
//! `v` (not with `??=`), or itself targeted `v` the same way. Null-aware
//! accesses (`v?.m()`) never join, and a statement whose arguments or
//! right-hand side read `v` breaks the chain, since a cascade would evaluate
//! them before the earlier member access. One diagnostic covers each run of
//! joinable statements, from the first statement that could be cascaded to
//! the last one.
//!
//! falcon must prove the receiver is a variable (a local, a parameter, an
//! instance field, or a top-level variable) rather than a type or import
//! prefix. For field and top-level-variable receivers, a closure, a call to
//! an unqualified function, or a bare identifier in the joined statements
//! breaks the chain, because it may reach the same storage.
//!
//! Adopted from the official `cascade_invocations` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, for_each_stmt_list, walk_expr};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct CascadeInvocations;

impl Rule for CascadeInvocations {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut receivers = Receivers::default();
        visit_program(&mut receivers, program, state);
        let mut diags = Vec::new();
        for_each_stmt_list(program, &mut |statements, _| {
            check_statements(statements, &receivers.kinds, ctx, &mut diags);
        });
        diags
    }
}

/// What a simple receiver identifier denotes, as far as falcon can prove.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReceiverKind {
    /// A local variable or parameter.
    Local,
    /// An instance field or top-level variable.
    Property,
}

/// Receiver classification, keyed by the start of the statement it targets.
#[derive(Default)]
struct Receivers {
    kinds: HashMap<usize, ReceiverKind>,
}

impl SemanticRuleVisitor for Receivers {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::Expr(statement) = node else {
            return;
        };
        let Some(name) = receiver_name(&statement.expr) else {
            return;
        };
        let kind = if state.environment.is_bound(name) {
            Some(ReceiverKind::Local)
        } else if state.enclosing_type.as_ref().is_some_and(|receiver| {
            state
                .signatures
                .resolved_field(receiver, name, &state.model)
                .is_some()
        }) {
            Some(ReceiverKind::Property)
        } else {
            state
                .model
                .resolve_value(name)
                .filter(|identity| state.signatures.function(identity).is_none())
                .map(|_| ReceiverKind::Property)
        };
        if let Some(kind) = kind {
            self.kinds.insert(statement.span.start, kind);
        }
    }
}

/// The simple identifier an expression statement targets, if any.
fn receiver_name(expr: &Expr) -> Option<&str> {
    let target = match expr {
        Expr::Assign { target, .. } => match target.as_ref() {
            Expr::Field {
                object,
                is_null_safe: false,
                ..
            } => object,
            other => return simple(other),
        },
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field {
                object,
                is_null_safe: false,
                ..
            } => object,
            _ => return None,
        },
        Expr::Cascade { object, .. } => object,
        Expr::Field {
            object,
            is_null_safe: false,
            ..
        } => object,
        _ => return None,
    };
    simple(target)
}

fn simple(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(identifier) => Some(&identifier.name),
        _ => None,
    }
}

/// One statement's role in a potential cascade.
struct Cascadable<'e> {
    element: Option<&'e str>,
    kind: Option<ReceiverKind>,
    /// Sub-expressions that must not read the receiver.
    critical: Vec<&'e Expr>,
    can_join: bool,
    can_receive: bool,
    can_be_cascaded: bool,
}

impl<'e> Cascadable<'e> {
    const NONE: Self = Self {
        element: None,
        kind: None,
        critical: Vec::new(),
        can_join: false,
        can_receive: false,
        can_be_cascaded: false,
    };

    fn from_statement(statement: &'e Stmt, kinds: &HashMap<usize, ReceiverKind>) -> Self {
        match statement {
            Stmt::LocalVar(declaration) => Self::from_declaration(declaration),
            Stmt::Expr(statement) => {
                Self::from_expression(&statement.expr, kinds.get(&statement.span.start).copied())
            }
            _ => Self::NONE,
        }
    }

    fn from_declaration(declaration: &'e LocalVarDecl) -> Self {
        let [declarator] = declaration.declarators.as_slice() else {
            return Self::NONE;
        };
        // `await` binds looser than it looks next to a cascade.
        if matches!(declarator.initializer, Some(Expr::Await { .. })) {
            return Self::NONE;
        }
        Self {
            element: Some(&declarator.name.name),
            kind: Some(ReceiverKind::Local),
            critical: declarator.initializer.iter().collect(),
            can_join: false,
            can_receive: declaration.keyword != VarKeyword::Const,
            can_be_cascaded: false,
        }
    }

    fn from_expression(expr: &'e Expr, kind: Option<ReceiverKind>) -> Self {
        match expr {
            Expr::Assign {
                target, op, value, ..
            } => match target.as_ref() {
                // `v = e` can receive a following cascade on `v`.
                Expr::Ident(identifier) => Self {
                    element: Some(&identifier.name),
                    kind,
                    critical: vec![value],
                    can_join: false,
                    can_receive: *op != AssignOp::NullCoalesceEq,
                    can_be_cascaded: false,
                },
                // `v.p = e` is a property set on `v`.
                Expr::Field {
                    object,
                    is_null_safe: false,
                    ..
                } if kind.is_some() => Self {
                    element: simple(object),
                    kind,
                    critical: vec![value],
                    can_join: true,
                    can_receive: *op != AssignOp::NullCoalesceEq,
                    can_be_cascaded: true,
                },
                _ => Self::NONE,
            },
            Expr::Call { callee, args, .. } => match callee.as_ref() {
                Expr::Field {
                    object,
                    is_null_safe: false,
                    ..
                } if kind.is_some() => Self {
                    element: simple(object),
                    kind,
                    critical: args
                        .positional
                        .iter()
                        .chain(args.named.iter().map(|argument| &argument.value))
                        .collect(),
                    can_join: true,
                    can_receive: true,
                    can_be_cascaded: true,
                },
                _ => Self::NONE,
            },
            Expr::Cascade {
                object, sections, ..
            } if kind.is_some() => {
                let mut critical = Vec::new();
                for section in sections {
                    for op in &section.ops {
                        match op {
                            CascadeOp::Field { .. } => {}
                            CascadeOp::Index { index, .. } => critical.push(index.as_ref()),
                            CascadeOp::Call(_, _, args) => critical.extend(
                                args.positional
                                    .iter()
                                    .chain(args.named.iter().map(|argument| &argument.value)),
                            ),
                            CascadeOp::Assign(target, _, value) => {
                                critical.push(target.as_ref());
                                critical.push(value.as_ref());
                            }
                        }
                    }
                }
                Self {
                    element: simple(object),
                    kind,
                    critical,
                    can_join: true,
                    can_receive: true,
                    can_be_cascaded: true,
                }
            }
            Expr::Field {
                object,
                is_null_safe: false,
                ..
            } if kind.is_some() => Self {
                element: simple(object),
                kind,
                critical: Vec::new(),
                can_join: true,
                can_receive: true,
                can_be_cascaded: true,
            },
            _ => Self::NONE,
        }
    }

    /// Whether this statement can be cascaded onto `previous`.
    fn compatible_with(&self, previous: &Cascadable<'_>) -> bool {
        let Some(element) = self.element else {
            return false;
        };
        previous.can_receive
            && self.can_join
            && (self.can_be_cascaded || previous.can_be_cascaded)
            && previous.element == Some(element)
            && !self.has_critical_dependencies(element, previous)
    }

    fn has_critical_dependencies(&self, element: &str, previous: &Cascadable<'_>) -> bool {
        let property = self.kind == Some(ReceiverKind::Property)
            || previous.kind == Some(ReceiverKind::Property);
        let mut finder = CriticalFinder {
            element,
            property,
            found: false,
        };
        for expr in self.critical.iter().chain(&previous.critical) {
            finder.visit_expr(expr);
        }
        finder.found
    }
}

/// Finds a read of the receiver (or, for field and top-level receivers, code
/// that may reach the same storage).
struct CriticalFinder<'e> {
    element: &'e str,
    property: bool,
    found: bool,
}

impl Visitor for CriticalFinder<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Ident(identifier) => {
                if identifier.name == self.element || self.property {
                    self.found = true;
                }
            }
            Expr::FuncExpr { .. } | Expr::This { .. } if self.property => self.found = true,
            _ => {}
        }
        walk_expr(self, node);
    }
}

fn check_statements(
    statements: &[Stmt],
    kinds: &HashMap<usize, ReceiverKind>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    if statements.len() < 2 {
        return;
    }
    let mut previous = Cascadable::NONE;
    let mut previous_end = None;
    let mut first_in_cascade: Option<usize> = None;
    for statement in statements {
        let current = Cascadable::from_statement(statement, kinds);
        if current.compatible_with(&previous) {
            first_in_cascade.get_or_insert(statement.span().start);
        } else if let (Some(start), Some(end)) = (first_in_cascade.take(), previous_end) {
            report(start, end, ctx, diags);
        }
        previous = current;
        previous_end = Some(statement.span().end);
    }
    if let (Some(start), Some(end)) = (first_in_cascade, previous_end) {
        report(start, end, ctx, diags);
    }
}

fn report(start: usize, end: usize, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    diags.push(ctx.diagnostic(
        "cascade-invocations",
        "Unnecessary duplication of receiver.",
        DiagSpan { start, end },
    ));
}
