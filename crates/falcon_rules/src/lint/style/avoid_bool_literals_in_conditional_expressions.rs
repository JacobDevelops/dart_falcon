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
use falcon_syntax::cst::{Expr, NodeId, Program, Span};

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidBoolLiteralsInConditionalExpressions;

impl Rule for AvoidBoolLiteralsInConditionalExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
    branches: HashMap<NodeId, usize>,
}

fn non_nullable_bool<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let ty = state.infer(expression);
    ty.interface("dart:core", "bool") && !ty.nullable()
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let collector = *self;
        let ctx = collector.ctx;
        collector
            .conditionals
            .into_iter()
            .filter(|conditional| conditional.bool_branches == 2)
            .map(|conditional| conditional.span)
            .map(|span| {
                ctx.diagnostic("Conditional expressions with a `bool` literal can be simplified.", span)
            })
            .collect()
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Some(index) = self.branches.remove(&node.syntax().id())
            && non_nullable_bool(node, state)
        {
            self.conditionals[index].bool_branches += 1;
        }
        let Expr::Conditional(conditional) = node else {
            return;
        };
        let then_expr = conditional.then_expr();
        let else_expr = conditional.else_expr();
        if !matches!(then_expr.unparenthesized(), Expr::BoolLit(_))
            && !matches!(else_expr.unparenthesized(), Expr::BoolLit(_))
        {
            return;
        }
        let index = self.conditionals.len();
        self.conditionals.push(Conditional {
            span: conditional.span(),
            bool_branches: 0,
        });
        self.branches.insert(then_expr.syntax().id(), index);
        self.branches.insert(else_expr.syntax().id(), index);
    }
}
