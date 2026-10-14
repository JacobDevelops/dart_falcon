//! Flags a doubly-negated boolean expression such as `!!x`.
//!
//! Two `!` operators cancel out, so `!!x` is just `x`, and a longer run of
//! leading bangs reduces to `x` or `!x` by parity. The redundant negation adds
//! no meaning and obscures the value. The rule flags a `!` whose operand is
//! itself a `!`, reporting once at the outer negation and skipping the inner
//! bangs. Despite the rule's name, it targets only stacked negations — it does
//! not rewrite `!(a == b)` into `a != b`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct AvoidInvertedBooleanExpressions;

impl Rule for AvoidInvertedBooleanExpressions {
    fn name(&self) -> &'static str {
        "avoid-inverted-boolean-expressions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Unary {
            op: UnaryOp::Bang,
            operand,
            span,
        } = node
            && is_bang_unary(operand)
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-inverted-boolean-expressions",
                "Avoid inverted boolean expressions. Simplify the double negation.",
                *span,
            ));
            // Resume below the whole bang chain so inner bangs are not re-flagged.
            let mut current = operand.as_ref();
            while let Expr::Unary {
                op: UnaryOp::Bang,
                operand: next,
                ..
            } = current
            {
                current = next.as_ref();
            }
            self.visit_expr(current);
            return;
        }
        walk_expr(self, node);
    }
}

/// True for a `!`-prefixed expression — the inner half of a `!!` double negation.
fn is_bang_unary(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Unary {
            op: UnaryOp::Bang,
            ..
        }
    )
}
