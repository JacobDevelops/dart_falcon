//! Ban configured dotted annotation names, without the leading `@`.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_banned_names::{Entry, entries, report};

#[derive(Default)]
pub struct AvoidBannedAnnotations;

impl Rule for AvoidBannedAnnotations {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = entries(ctx, "avoid-banned-annotations");
        if entries.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            entries,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: Vec<Entry<'a>>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_annotation(&mut self, node: Annotation<'a>) -> StructuralTraversal {
        let mut name = String::with_capacity(node.span().end - node.span().start);
        for part in node.name() {
            if !name.is_empty() {
                name.push('.');
            }
            name.push_str(part.name());
        }
        if let Some(constructor) = node.constructor_name() {
            name.push('.');
            name.push_str(constructor.name());
        }
        report(self.ctx, &self.entries, &name, node.span(), &mut self.diagnostics);
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
