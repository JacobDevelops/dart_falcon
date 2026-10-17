//! Flags an expression statement that is a cascade with a single section.
//!
//! A cascade (`..`) exists to chain several operations on one receiver; with a
//! single section it buys nothing over an ordinary member access and only
//! obscures intent. Rewrite `foo..bar()` as `foo.bar()`. Cascades with two or
//! more sections are the idiomatic form and are not flagged.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidSingleCascadeInExpressionStatements;

impl Rule for AvoidSingleCascadeInExpressionStatements {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
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
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::Expr(e) = node
            && let Expr::Cascade { sections, span, .. } = e.expr.unparenthesized()
            && sections.len() == 1
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-single-cascade-in-expression-statements",
                "Single cascade in an expression statement — use `.` instead of `..`",
                *span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
