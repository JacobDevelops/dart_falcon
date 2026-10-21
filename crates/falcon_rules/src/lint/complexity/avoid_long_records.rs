//! Reports record values and record types with too many fields.
//!
//! `max_fields` (integer, default: 6) limits positional and named fields together.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidLongRecords;
impl Rule for AvoidLongRecords {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            threshold: crate::lint::options::usize(
                crate::lint::options::for_rule(ctx, AvoidLongRecords::NAME),
                "max_fields",
            )
            .unwrap_or(6),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    threshold: usize,
}
impl Collector<'_> {
    fn check(&mut self, count: usize, span: Span) {
        if count > self.threshold {
            self.diags.push(self.ctx.diagnostic(format!(
                "Record has {count} fields (max {}). Replace it with a named class.",
                self.threshold
            ), span));
        }
    }
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Record(record) = node { self.check(record.fields().count(), record.span()); }
        StructuralTraversal::Continue
    }
    fn visit_record_type(&mut self, node: RecordType<'a>) -> StructuralTraversal {
        self.check(node.positional().count() + node.named().count(), node.span());
        StructuralTraversal::Continue
    }
}
}
