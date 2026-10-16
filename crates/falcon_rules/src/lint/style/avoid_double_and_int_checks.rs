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
//! resolving to the non-nullable `dart:core` classes. Like the official lint,
//! this matches `is!` tests too; it compares the types, not the test polarity.
//!
//! Adopted from the official `avoid_double_and_int_checks` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use falcon_analyze::resolve::lexical_usage::locally_bound_identifiers;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidDoubleAndIntChecks;

impl Rule for AvoidDoubleAndIntChecks {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diagnostics: Vec::new(),
            ctx,
            bindings: locally_bound_identifiers(program),
        }))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    bindings: HashSet<usize>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

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
        let (IfCondition::Expr(if_condition), IfCondition::Expr(else_condition)) =
            (&statement.condition, &else_if.condition)
        else {
            return;
        };
        let (
            Expr::Is {
                expr: if_subject,
                dart_type: if_type,
                ..
            },
            Expr::Is {
                expr: else_subject,
                dart_type: else_type,
                ..
            },
        ) = (
            if_condition.unparenthesized(),
            else_condition.unparenthesized(),
        )
        else {
            return;
        };
        let (Expr::Ident(if_name), Expr::Ident(else_name)) =
            (if_subject.unparenthesized(), else_subject.unparenthesized())
        else {
            return;
        };
        if if_name.name != else_name.name
            || !self.bindings.contains(&if_name.span.start)
            || !is_core_type(if_type, "double", state)
            || !is_core_type(else_type, "int", state)
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-double-and-int-checks",
            "Explicit check for double or int.",
            *else_condition.unparenthesized().span(),
        ));
    }
}

fn is_core_type(ty: &DartType, name: &'static str, state: &SemanticState<'_>) -> bool {
    state.model.resolve_type_in(ty, &state.type_parameters) == state.model.core_type(name, false)
}
