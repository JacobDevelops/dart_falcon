//! Flags boolean expressions with more logical operands than the configured threshold.
//!
//! Each operand joined by `&&` or `||` counts as one; parentheses and unary
//! operators do not hide logical operands. Calls and comparisons are opaque
//! operands, so callback bodies do not inflate their caller's complexity.
//! Only the outermost connected logical expression is reported. Logical
//! expressions inside calls and callbacks are checked independently.
//!
//! ## Options
//!
//! `threshold` (integer, default: 10) — maximum number of logical operands.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidComplexConditions;

impl Rule for AvoidComplexConditions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Conditions {
            ctx,
            threshold: crate::lint::options::usize(
                crate::lint::options::for_rule(ctx, "avoid-complex-conditions"),
                "threshold",
            )
            .unwrap_or(10),
            in_logical_tree: false,
            parents: Vec::new(),
            diags: Vec::new(),
        }))
    }
}

pub(super) fn logical_operands(expression: Expr<'_>) -> usize {
    use falcon_syntax::cst::visit::{Visitor, walk_expr};
    struct Operands(usize);
    impl<'a> Visitor<'a> for Operands {
        fn visit_expr(&mut self, node: Expr<'a>) {
            match node {
                Expr::Binary(binary) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or) => {
                    walk_expr(self, node);
                }
                Expr::Paren(_) | Expr::Unary(_) => walk_expr(self, node),
                _ => self.0 += 1,
            }
        }
    }
    let mut operands = Operands(0);
    operands.visit_expr(expression);
    operands.0
}

struct Conditions<'a> {
    ctx: &'a AnalyzeContext<'a>,
    threshold: usize,
    in_logical_tree: bool,
    parents: Vec<bool>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Conditions<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.parents.push(self.in_logical_tree);
        if matches!(node, Expr::Binary(binary) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or)) {
            if !self.in_logical_tree {
                let count = logical_operands(node);
                if count > self.threshold {
                    self.diags.push(self.ctx.diagnostic("avoid-complex-conditions", format!("Condition has {count} logical operands (max {}). Extract a named condition.", self.threshold), node.span()));
                }
            }
            self.in_logical_tree = true;
        } else if !matches!(node, Expr::Paren(_) | Expr::Unary(_)) {
            self.in_logical_tree = false;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.in_logical_tree = self.parents.pop().unwrap_or(false);
    }
}
}
