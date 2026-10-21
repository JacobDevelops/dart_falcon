//! Split chained assignments into separate statements.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMultiAssignment;

impl Rule for AvoidMultiAssignment {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Assign(assign) = node
            && matches!(assign.value().unparenthesized(), Expr::Assign(_))
        {
            // Report once for a complete assignment chain, while still visiting
            // unrelated assignments in its targets and values.
            let mut parent = assign.syntax().parent();
            while parent.is_some_and(|node| node.kind() == NodeKind::ParenExpr) {
                parent = parent.and_then(|node| node.parent());
            }
            let chained = parent.is_some_and(|parent| {
                let value = match (Expr::cast(parent), CascadeOp::cast(parent)) {
                    (Some(Expr::Assign(outer)), _) => Some(outer.value()),
                    (_, Some(CascadeOp::Assign(outer))) => Some(outer.value()),
                    (_, Some(CascadeOp::IndexAssign(outer))) => Some(outer.value()),
                    _ => None,
                };
                value.is_some_and(|value| value.unparenthesized().syntax() == assign.syntax())
            });
            if !chained {
                self.diags.push(self.ctx.diagnostic(
                    "Split chained assignments into separate statements.", assign.span(),
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_cascade_op(&mut self, node: CascadeOp<'a>) -> StructuralTraversal {
        let value = match node {
            CascadeOp::Assign(assign) => Some(assign.value()),
            CascadeOp::IndexAssign(assign) => Some(assign.value()),
            _ => None,
        };
        if value.is_some_and(|value| matches!(value.unparenthesized(), Expr::Assign(_))) {
            self.diags.push(self.ctx.diagnostic(
                "Split chained assignments into separate statements.", node.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
