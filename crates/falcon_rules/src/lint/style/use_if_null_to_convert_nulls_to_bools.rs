//! Prefers `??` to comparing a nullable `bool` against a boolean literal.
//!
//! For a `bool?` value `b`, `b == true` is `b ?? false` and `b != false` is
//! `b ?? true`; the if-null forms state the null default directly. The rule
//! fires on `==` with `true` and `!=` with `false` on the right when the left
//! operand's static type is proven to be `bool?`.
//!
//! Falcon has no flow analysis, so a left operand mentioning a variable or
//! field that could be type-promoted anywhere in the file (`== null` /
//! `!= null` checks, `is` tests, `!`, assignments, pattern subjects) is left
//! alone: after `if (flag == null) return;`, `flag` is a non-nullable `bool`.
//!
//! Adopted from the official `use_if_null_to_convert_nulls_to_bools` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UseIfNullToConvertNullsToBools;

impl Rule for UseIfNullToConvertNullsToBools {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut promotable = PromotableNames::default();
        promotable.visit_program(program);
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            promoted: promotable.names,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    promoted: HashSet<String>,
}

fn is_nullable_bool(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Interface { identity, nullable: true, .. }
        if identity.is_sdk("dart:core", "bool"))
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op,
            left,
            right,
            span,
        } = node
        else {
            return;
        };
        let matches = matches!(
            (op, right.as_ref()),
            (BinaryOp::EqEq, Expr::BoolLit { value: true, .. })
                | (BinaryOp::NotEq, Expr::BoolLit { value: false, .. })
        );
        if matches && is_nullable_bool(&state.infer(left)) && !self.mentions_promoted(left) {
            self.diags.push(self.ctx.diagnostic(
                "use-if-null-to-convert-nulls-to-bools",
                "Use an if-null operator to convert a 'null' to a 'bool'.",
                *span,
            ));
        }
    }
}

impl Collector<'_> {
    fn mentions_promoted(&self, expression: &Expr) -> bool {
        let mut mentions = Mentions {
            promoted: &self.promoted,
            found: false,
        };
        mentions.visit_expr(expression);
        mentions.found
    }
}

/// Names of variables and fields that appear anywhere in the file in a position
/// that can promote a `bool?` to `bool`, keyed by the final name segment.
/// Comparing against `true` or `false` does not promote, so only comparisons
/// with `null` count.
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
                if matches!(right.as_ref(), Expr::NullLit { .. }) {
                    self.record(left);
                }
                if matches!(left.as_ref(), Expr::NullLit { .. }) {
                    self.record(right);
                }
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
            // A local's initializer does not promote it: `bool? x = true;`
            // keeps the declared `bool?`, and `var x = e` has the type of `e`.
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
