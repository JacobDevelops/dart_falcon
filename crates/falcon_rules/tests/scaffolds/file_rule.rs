use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Expr, Program};

#[derive(Default)]
pub struct ScaffoldRule;

impl Rule for ScaffoldRule {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let _ = (program, ctx);
        RulePreparation::Structural(Box::new(Collector {
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector {
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        let _ = node;
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
