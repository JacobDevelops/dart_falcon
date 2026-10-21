//! Flags a doubly-negated boolean expression such as `!!x`.
//!
//! Two `!` operators cancel out, so `!!x` is just `x`, and a longer run of
//! leading bangs reduces to `x` or `!x` by parity. The redundant negation adds
//! no meaning and obscures the value. The rule flags a `!` whose operand is
//! itself a `!`, reporting once at the outer negation and skipping the inner
//! bangs. Despite the rule's name, it targets only stacked negations — it does
//! not rewrite `!(a == b)` into `a != b`.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidInvertedBooleanExpressions;

impl Rule for AvoidInvertedBooleanExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            skipping_chain: false,
            chain_contexts: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    skipping_chain: bool,
    chain_contexts: Vec<bool>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.chain_contexts.push(self.skipping_chain);
        if self.skipping_chain && (is_bang_unary(node) || matches!(node, Expr::Paren(_))) {
            return StructuralTraversal::Continue;
        }
        self.skipping_chain = false;
        if let Expr::Unary(unary) = node
            && unary.op() == UnaryOp::Bang
            && is_bang_unary(unary.operand())
        {
            self.diags.push(self.ctx.diagnostic(
                "Avoid inverted boolean expressions. Simplify the double negation.",
                unary.span(),
            ));
            self.skipping_chain = true;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.skipping_chain = self.chain_contexts.pop().unwrap();
    }
}
}

/// True for a `!`-prefixed expression — the inner half of a `!!` double negation.
fn is_bang_unary(expr: Expr<'_>) -> bool {
    matches!(expr.unparenthesized(), Expr::Unary(unary) if unary.op() == UnaryOp::Bang)
}
