//! Reports arithmetic expressions containing too many terms.
//!
//! `max_terms` (integer, default: 6) limits connected +, -, *, /, ~/, and %
//! operands. Parentheses do not reset the count; calls are opaque operands.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidComplexArithmeticExpressions;
impl Rule for AvoidComplexArithmeticExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            threshold: crate::lint::options::usize(
                crate::lint::options::for_rule(ctx, "avoid-complex-arithmetic-expressions"),
                "max_terms",
            )
            .unwrap_or(6),
            parents: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    threshold: usize,
    parents: Vec<(Span, bool)>,
}
fn arithmetic<'a>(node: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let Expr::Binary(binary) = node else {
        return false;
    };
    matches!(
        binary.op(),
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::IntDiv
            | BinaryOp::Mod
    ) && [binary.left(), binary.right()].into_iter().all(|operand| {
        let ty = state.infer(operand);
        !ty.nullable()
            && ["num", "int", "double"]
                .into_iter()
                .any(|name| ty.interface("dart:core", name))
    })
}

fn operations<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> usize {
    use falcon_syntax::cst::visit::{Visitor, walk_expr};
    struct Operations<'s, 'a> {
        count: usize,
        state: &'s SemanticState<'a>,
    }
    impl<'a> Visitor<'a> for Operations<'_, 'a> {
        fn visit_expr(&mut self, node: Expr<'a>) {
            if arithmetic(node, self.state) {
                self.count += 1;
                walk_expr(self, node);
            } else if matches!(node, Expr::Paren(_) | Expr::Unary(_)) {
                walk_expr(self, node);
            }
        }
    }
    let mut counter = Operations { count: 0, state };
    counter.visit_expr(expression);
    counter.count
}
impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let span = node.span();
        // Semantic traversal is preorder; disjoint spans mark completed parents.
        while self
            .parents
            .last()
            .is_some_and(|(parent, _)| parent.end <= span.start)
        {
            self.parents.pop();
        }
        let connected = self.parents.last().is_some_and(|(_, connected)| *connected);
        let numeric = arithmetic(node, state);
        if numeric && !connected {
            let count = operations(node, state) + 1;
            if count > self.threshold {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-complex-arithmetic-expressions",
                    format!("Expression has {count} arithmetic terms (max {}). Extract intermediate values.", self.threshold),
                    span,
                ));
            }
        }
        let transparent = matches!(node, Expr::Paren(_) | Expr::Unary(_));
        self.parents
            .push((span, numeric || (transparent && connected)));
    }
}
