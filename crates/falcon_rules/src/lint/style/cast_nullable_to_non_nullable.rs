//! Flags `e as T` casts of a nullable value to a non-nullable type.
//!
//! Casting a nullable expression straight to a non-nullable type hides the null
//! check inside the cast; the null assertion operator `!` states the intent
//! directly. The cast is reported only when the operand's static type is proven
//! nullable (never `dynamic` or unresolved) and the target type is proven
//! non-nullable (not a potentially nullable type parameter, `dynamic`, `void`,
//! `Null`, or `FutureOr` of a nullable type). Falcon has no flow analysis, so an
//! operand mentioning a variable or field that could be type-promoted anywhere
//! in the file (null checks, `is` tests, `!`, assignments, initialized local
//! declarations, pattern subjects) is left alone rather than risk reporting a
//! promoted, non-nullable value.
//!
//! ```dart
//! String? s;
//! var a = s as String; // flagged
//! var b = s!;          // ok
//! ```
//!
//! Adopted from the official `cast_nullable_to_non_nullable` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::semantic_type_operations::nullability;

#[derive(Default)]
pub struct CastNullableToNonNullable;

impl Rule for CastNullableToNonNullable {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut promotable = PromotableNames::default();
        promotable.visit_program(program);
        let mut collector = Collector {
            ctx,
            promoted: promotable.names,
            diagnostics: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    promoted: HashSet<String>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::As {
            expr,
            dart_type,
            span,
        } = node
        else {
            return;
        };
        let target = state
            .model
            .resolve_type_in(dart_type, &state.type_parameters);
        if nullability(&target, &state.signatures, self.ctx.types) != Truth::No {
            return;
        }
        if nullability(&state.infer(expr), &state.signatures, self.ctx.types) != Truth::Yes {
            return;
        }
        let mut mentions = Mentions {
            promoted: &self.promoted,
            found: false,
        };
        mentions.visit_expr(expr);
        if mentions.found {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "cast-nullable-to-non-nullable",
            "Don't cast a nullable value to a non-nullable type.",
            *span,
        ));
    }
}

/// Names of variables and fields that appear anywhere in the file in a position
/// that can promote their type, keyed by the final name segment.
#[derive(Default)]
struct PromotableNames {
    names: HashSet<String>,
}

impl PromotableNames {
    fn record(&mut self, expression: &Expr) {
        match expression {
            Expr::Ident(identifier) => {
                self.names.insert(identifier.name.clone());
            }
            Expr::Field { field, .. } => {
                self.names.insert(field.name.clone());
            }
            _ => {}
        }
    }
}

impl Visitor for PromotableNames {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Binary {
                op: BinaryOp::EqEq | BinaryOp::NotEq,
                left,
                right,
                ..
            } => {
                self.record(left);
                self.record(right);
            }
            Expr::Is { expr, .. } | Expr::NullAssert { operand: expr, .. } => self.record(expr),
            Expr::Assign { target, .. } => self.record(target),
            Expr::Switch { subject, .. } => self.record(subject),
            Expr::Cascade { sections, .. } => {
                for section in sections {
                    for op in &section.ops {
                        if let CascadeOp::Assign(target, _, _) = op {
                            self.record(target);
                        }
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::If(statement) => {
                if let IfCondition::Case(subject, _, _) = &statement.condition {
                    self.record(subject);
                }
            }
            Stmt::Switch(statement) => self.record(&statement.subject),
            // Initialization promotes like an assignment (`int? x = 5;`).
            Stmt::LocalVar(declaration) => {
                for declarator in &declaration.declarators {
                    if declarator.initializer.is_some() {
                        self.names.insert(declarator.name.name.clone());
                    }
                }
            }
            _ => {}
        }
        walk_stmt(self, node);
    }
}

/// Whether an expression mentions any possibly promoted name.
struct Mentions<'a> {
    promoted: &'a HashSet<String>,
    found: bool,
}

impl Visitor for Mentions<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        let name = match node {
            Expr::Ident(identifier) => Some(&identifier.name),
            Expr::Field { field, .. } => Some(&field.name),
            _ => None,
        };
        if name.is_some_and(|name| self.promoted.contains(name)) {
            self.found = true;
            return;
        }
        walk_expr(self, node);
    }
}
