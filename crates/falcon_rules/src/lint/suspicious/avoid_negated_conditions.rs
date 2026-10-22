//! Prefer positive conditions when both branches can be swapped.
//! Checks if statements, conditional expressions, and collection/map if/else elements.
//! `ignore-null-checks` (default false) exempts comparisons with null.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Exempt null comparisons from the negated-condition diagnostic.
    ignore_null_checks: bool,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct AvoidNegatedConditions;

impl Rule for AvoidNegatedConditions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let ignore_null_checks =
            crate::lint::options::get::<Options>(ctx, Self::NAME).ignore_null_checks;
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
            self.diags.push(self.ctx.diagnostic("Prefer a positive condition and swap the branches.", condition.span()));
        }
    }

    fn element<'a, E: Element<'a>>(&mut self, node: E) -> StructuralTraversal {
        if let ElementKind::If {
            condition: IfCondition::Expr(condition),
            else_element: Some(other),
            ..
        } = node.kind()
            && !matches!(
                other.syntax().kind(),
                NodeKind::IfElement | NodeKind::MapIfElement
            )
        {
            self.check(condition);
        }
        StructuralTraversal::Continue
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
        self.element(node)
    }

    fn visit_map_element(&mut self, node: MapElement<'a>) -> StructuralTraversal {
        self.element(node)
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Conditional(conditional) = node {
            self.check(conditional.condition());
        }
        StructuralTraversal::Continue
    }
}
}
