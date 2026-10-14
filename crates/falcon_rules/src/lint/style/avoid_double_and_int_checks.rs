//! Avoid checking a value for `double` and then for `int`.
//!
//! When compiled to JavaScript every number is a double, so integral values
//! pass an `is double` test and an `else if (x is int)` branch is never reached
//! there:
//!
//! ```dart
//! if (x is double) {
//!   handleDouble(x);
//! } else if (x is int) { // BAD
//!   handleInt(x);
//! }
//!
//! if (x is int) { // GOOD: check `int` first
//!   handleInt(x);
//! } else if (x is double) {
//!   handleDouble(x);
//! }
//! ```
//!
//! Reported when an `if (x is double)` statement's `else` branch is directly
//! `if (x is int)` on the same parameter or local variable, with both types
//! resolving to the non-nullable `dart:core` classes.
//!
//! Adopted from the official `avoid_double_and_int_checks` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidDoubleAndIntChecks;

impl Rule for AvoidDoubleAndIntChecks {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::If(statement) = node else {
            return;
        };
        let Some(else_branch) = &statement.else_branch else {
            return;
        };
        let Stmt::If(else_if) = else_branch.as_ref() else {
            return;
        };
        let (
            IfCondition::Expr(Expr::Is {
                expr: if_subject,
                dart_type: if_type,
                negated: false,
                ..
            }),
            IfCondition::Expr(
                else_condition @ Expr::Is {
                    expr: else_subject,
                    dart_type: else_type,
                    negated: false,
                    ..
                },
            ),
        ) = (&statement.condition, &else_if.condition)
        else {
            return;
        };
        let (Expr::Ident(if_name), Expr::Ident(else_name)) =
            (if_subject.as_ref(), else_subject.as_ref())
        else {
            return;
        };
        if if_name.name != else_name.name
            // Only parameters and local variables live in the environment.
            || !state.environment.is_bound(&if_name.name)
            || !is_core_type(if_type, "double", state)
            || !is_core_type(else_type, "int", state)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-double-and-int-checks",
            "Explicit check for double or int.",
            *else_condition.span(),
        ));
    }
}

fn is_core_type(ty: &DartType, name: &'static str, state: &SemanticState<'_>) -> bool {
    state.model.resolve_type_in(ty, &state.type_parameters) == state.model.core_type(name, false)
}
