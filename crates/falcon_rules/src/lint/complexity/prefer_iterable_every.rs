//! Flags `!iterable.where(...).isEmpty` and `iterable.where(...).length == other.length`.
//!
//! Both patterns ask whether every element satisfies a predicate;
//! `iterable.every(...)` says so directly and short-circuits on the first
//! failure. The rule matches a negated `.where(...).isEmpty` and a
//! `.where(...).length` compared for equality against a `.length`.

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
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

fn is_negated_where_is_empty(expr: Expr<'_>) -> Option<Span> {
    if let Expr::Unary(unary) = expr.unparenthesized()
        && unary.op() == UnaryOp::Bang
        && let Expr::Field(field) = unary.operand().unparenthesized()
        && field.field().name() == "isEmpty"
        && let Expr::Call(call) = field.object().unparenthesized()
        && let Expr::Field(where_field) = call.callee().unparenthesized()
        && where_field.field().name() == "where"
    {
        return Some(unary.span());
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
        if let Some(span) =
            is_negated_where_is_empty(node).or_else(|| is_where_length_eq_length(node))
        {
            self.diags.push(self.ctx.diagnostic(
                "Use .every() instead of .where().isEmpty or .where().length comparison.",
                span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
