//! Flags `iterable.where(predicate).isEmpty` and filtered-length comparisons.
//!
//! An empty filtered iterable means every element fails the predicate. Prefer
//! `iterable.every((element) => !predicate(element))` to express that condition
//! directly. When a filtered length equals the original iterable's length, use
//! `iterable.every(predicate)` instead. Both forms can stop at the first failure.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferIterableEvery;

impl Rule for PreferIterableEvery {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

fn is_where_is_empty(expr: Expr<'_>) -> Option<Span> {
    if let Expr::Field(field) = expr.unparenthesized()
        && field.field().name() == "isEmpty"
        && let Expr::Call(call) = field.object().unparenthesized()
        && let Expr::Field(where_field) = call.callee().unparenthesized()
        && where_field.field().name() == "where"
    {
        let mut parent = expr.syntax().parent();
        while parent
            .and_then(Expr::cast)
            .is_some_and(|node| matches!(node, Expr::Paren(_)))
        {
            parent = parent.and_then(|node| node.parent());
        }
        if matches!(parent.and_then(Expr::cast), Some(Expr::Unary(unary)) if unary.op() == UnaryOp::Bang)
        {
            return None;
        }
        return Some(field.span());
    }
    None
}

fn is_where_length_eq_length(expr: Expr<'_>) -> Option<Span> {
    if let Expr::Binary(binary) = expr.unparenthesized()
        && binary.op() == BinaryOp::EqEq
        && let Expr::Field(left) = binary.left().unparenthesized()
        && left.field().name() == "length"
        && let Expr::Call(call) = left.object().unparenthesized()
        && let Expr::Field(where_field) = call.callee().unparenthesized()
        && where_field.field().name() == "where"
        && let Expr::Field(right) = binary.right().unparenthesized()
        && right.field().name() == "length"
    {
        return Some(binary.span());
    }
    None
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the syntax tree, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::Paren(_)) {
            return StructuralTraversal::Continue;
        }
        let matched = is_where_is_empty(node)
            .map(|span| (span, "Use `every((element) => !predicate(element))` instead of `where(predicate).isEmpty`."))
            .or_else(|| is_where_length_eq_length(node)
                .map(|span| (span, "Use `every(predicate)` instead of comparing the filtered and original iterable lengths.")));
        if let Some((span, message)) = matched {
            self.diags.push(self.ctx.diagnostic(
                message,
                span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
