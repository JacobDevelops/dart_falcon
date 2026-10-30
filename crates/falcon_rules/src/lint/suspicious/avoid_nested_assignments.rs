//! Require assignments to be standalone statements, arrow bodies or loop operations.
//! `ignore-null-aware-assignments` (default false) exempts `??=`.
//! Conditions and chained assignments are handled by their dedicated rules.

use crate::lint::support::condition_position::is_in_condition;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Exempt null-aware assignments (`??=`) from the nested-assignment diagnostic.
    ignore_null_aware_assignments: bool,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct AvoidNestedAssignments;

impl Rule for AvoidNestedAssignments {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let ignore_null_aware =
            crate::lint::options::get::<Options>(ctx, Self::NAME).ignore_null_aware_assignments;
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            ignore_null_aware,
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    ignore_null_aware: bool,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Assign(assign) = node
            && !(self.ignore_null_aware && assign.op() == AssignOp::NullCoalesceEq)
            && !is_in_condition(assign.syntax())
            && !matches!(assign.value().unparenthesized(), Expr::Assign(_))
        {
            let mut parent = assign.syntax().parent();
            while parent.is_some_and(|node| node.kind() == NodeKind::ParenExpr) {
                parent = parent.and_then(SyntaxNode::parent);
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
            if !chained && parent.is_some_and(|node| !matches!(node.kind(),
                NodeKind::ExprStmt | NodeKind::ArrowBody | NodeKind::ForExprsInit | NodeKind::ForUpdates))
            {
                self.diags.push(self.ctx.diagnostic(
                    "Move this assignment out of the surrounding expression.", assign.span(),
                ));
            }
        }
        StructuralTraversal::Continue
    }
}
}
