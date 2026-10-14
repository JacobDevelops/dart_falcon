//! Flags conditional expressions over `bool` that have a `bool` literal branch.
//!
//! When both branches of `c ? a : b` are non-nullable `bool`s and one is a
//! literal, the expression simplifies to a logical operator: `c ? true : b` is
//! `c || b`, `c ? false : b` is `!c && b`, `c ? b : true` is `!c || b`, and
//! `c ? b : false` is `c && b`.
//!
//! ```dart
//! final ok = ready ? true : retry; // flagged
//! final ok = ready || retry;       // ok
//! ```
//!
//! Adopted from the official `avoid_bool_literals_in_conditional_expressions`
//! lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::Expr;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidBoolLiteralsInConditionalExpressions;

impl Rule for AvoidBoolLiteralsInConditionalExpressions {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

fn non_nullable_bool(expression: &Expr, state: &SemanticState<'_>) -> bool {
    let ty = state.infer(expression);
    ty.interface("dart:core", "bool") && !ty.nullable()
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Conditional {
            then_expr,
            else_expr,
            span,
            ..
        } = node
        else {
            return;
        };
        let has_literal = [then_expr, else_expr]
            .into_iter()
            .any(|branch| matches!(branch.as_ref(), Expr::BoolLit { .. }));
        if has_literal && non_nullable_bool(then_expr, state) && non_nullable_bool(else_expr, state)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-bool-literals-in-conditional-expressions",
                "Conditional expressions with a 'bool' literal can be simplified.",
                *span,
            ));
        }
    }
}
