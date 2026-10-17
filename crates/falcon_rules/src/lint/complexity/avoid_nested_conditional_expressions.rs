//! Flags a ternary conditional expression nested inside another ternary.
//!
//! Stacked conditionals like `a ? b : c ? d : e` are hard to read and easy to
//! misparse; an `if`/`else` chain, a `switch` expression, or an extracted
//! variable makes the branching explicit. The rule reports any conditional
//! expression that has another conditional as an ancestor (nesting level two or
//! deeper), matching dart_code_linter's default acceptable level of one.
//! Closure bodies restart the nesting count.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidNestedConditionalExpressions;

impl Rule for AvoidNestedConditionalExpressions {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            cond_ancestor: false,
            ancestors: Vec::new(),
        }))
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    cond_ancestor: bool,
    ancestors: Vec<bool>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.ancestors.push(self.cond_ancestor);
        if matches!(node, Expr::FuncExpr { .. }) {
            self.cond_ancestor = false;
        }
        if let Expr::Conditional { span, .. } = node {
            if self.cond_ancestor {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-nested-conditional-expressions",
                    "Avoid nested conditional expressions",
                    *span,
                ));
            }
            self.cond_ancestor = true;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.cond_ancestor = self.ancestors.pop().unwrap();
    }
}
}
