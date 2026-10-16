//! Flags conditional expressions over `bool` that have a `bool` literal branch.
//!
//! When both branches of `c ? a : b` are non-nullable `bool`s and one is a
//! literal, the expression simplifies to a logical operator: `c ? true : b` is
//! `c || b`, `c ? false : b` is `!c && b`, `c ? b : true` is `!c || b`, and
//! `c ? b : false` is `c && b`.
//! Branches are checked in their own flow state, after condition promotions.
//! Reports once per conditional: the analyzer deduplicates upstream reports
//! when both branches are literals at the same diagnostic location.
//!
//! ```dart
//! final ok = ready ? true : retry; // flagged
//! final ok = ready || retry;       // ok
//! ```
//!
//! Adopted from the official `avoid_bool_literals_in_conditional_expressions`
//! lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::{Expr, Span};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidBoolLiteralsInConditionalExpressions;

impl Rule for AvoidBoolLiteralsInConditionalExpressions {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            conditionals: Vec::new(),
            branches: HashMap::new(),
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Conditional {
    span: Span,
    bool_branches: usize,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    conditionals: Vec<Conditional>,
    /// Branches are typed when the shared visitor reaches their promoted scope.
    branches: HashMap<*const Expr, usize>,
}

fn non_nullable_bool(expression: &Expr, state: &SemanticState<'_>) -> bool {
    let ty = state.infer(expression);
    ty.interface("dart:core", "bool") && !ty.nullable()
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let collector = *self;
        let ctx = collector.ctx;
        collector
            .conditionals
            .into_iter()
            .filter(|conditional| conditional.bool_branches == 2)
            .map(|conditional| conditional.span)
            .map(|span| {
                ctx.diagnostic(
                    "avoid-bool-literals-in-conditional-expressions",
                    "Conditional expressions with a 'bool' literal can be simplified.",
                    span,
                )
            })
            .collect()
    }
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Some(index) = self.branches.remove(&std::ptr::from_ref(node))
            && non_nullable_bool(node, state)
        {
            self.conditionals[index].bool_branches += 1;
        }
        let Expr::Conditional {
            then_expr,
            else_expr,
            span,
            ..
        } = node
        else {
            return;
        };
        if !matches!(then_expr.unparenthesized(), Expr::BoolLit { .. })
            && !matches!(else_expr.unparenthesized(), Expr::BoolLit { .. })
        {
            return;
        }
        let index = self.conditionals.len();
        self.conditionals.push(Conditional {
            span: *span,
            bool_branches: 0,
        });
        self.branches
            .insert(std::ptr::from_ref(then_expr.as_ref()), index);
        self.branches
            .insert(std::ptr::from_ref(else_expr.as_ref()), index);
    }
}
