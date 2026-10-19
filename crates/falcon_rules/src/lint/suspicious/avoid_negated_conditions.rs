//! Prefer positive conditions when both branches can be swapped.
//! Checks if statements, conditional expressions, and collection/map if/else elements.
//! `ignore_null_checks` (default false) exempts comparisons with null.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidNegatedConditions;

impl Rule for AvoidNegatedConditions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let ignore_null_checks = crate::lint::options::for_rule(ctx, "avoid-negated-conditions")
            .and_then(|options| options.get("ignore_null_checks"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            ignore_null_checks,
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    ignore_null_checks: bool,
}

impl Collector<'_> {
    fn check(&mut self, condition: Expr<'_>) {
        let negated = match condition.unparenthesized() {
            Expr::Unary(unary) => unary.op() == UnaryOp::Bang,
            Expr::Is(test) => test.negated(),
            Expr::Binary(binary) => {
                binary.op() == BinaryOp::NotEq
                    && !(self.ignore_null_checks
                        && (matches!(binary.left().unparenthesized(), Expr::NullLit(_))
                            || matches!(binary.right().unparenthesized(), Expr::NullLit(_))))
            }
            _ => false,
        };
        if negated {
            self.diags.push(self.ctx.diagnostic(
                "avoid-negated-conditions",
                "Prefer a positive condition and swap the branches.",
                condition.span(),
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(statement) = node
            && statement.else_branch().is_some()
            && !matches!(statement.else_branch(), Some(Stmt::If(_)))
            && let IfCondition::Expr(condition) = statement.condition()
        {
            self.check(condition);
        }
        StructuralTraversal::Continue
    }

    fn visit_collection_element(&mut self, node: CollectionElement<'a>) -> StructuralTraversal {
        if let CollectionElement::If(element) = node
            && element.else_elem().is_some()
            && !matches!(element.else_elem(), Some(CollectionElement::If(_)))
            && let IfCondition::Expr(condition) = element.condition()
        {
            self.check(condition);
        }
        StructuralTraversal::Continue
    }

    fn visit_map_element(&mut self, node: MapElement<'a>) -> StructuralTraversal {
        if let MapElement::If(element) = node
            && element.else_entry().is_some()
            && !matches!(element.else_entry(), Some(MapElement::If(_)))
            && let IfCondition::Expr(condition) = element.condition()
        {
            self.check(condition);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Conditional(conditional) = node {
            self.check(conditional.condition());
        }
        StructuralTraversal::Continue
    }
}
}
