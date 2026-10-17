//! Flags a `Container` whose only argument is a `child`.
//!
//! A `Container` that wraps nothing but a `child` adds a widget to the tree
//! that neither paints, sizes, nor positions anything — it is pure overhead.
//! Dropping it and using the child directly shrinks the widget tree, which
//! helps rebuild performance and readability. The rule fires only when
//! `Container` has exactly one named argument, `child`, and no positional
//! arguments.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessaryContainers;

impl Rule for AvoidUnnecessaryContainers {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Some((name, args, span)) = crate::lint::flutter::widget_construction(node)
            && name == "Container"
            && args.positional().is_empty()
            && args.named().len() == 1
            && args.named().next().unwrap().name().name() == "child"
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-unnecessary-containers",
                "Avoid a Container with only a child; use the child directly.",
                span,
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
