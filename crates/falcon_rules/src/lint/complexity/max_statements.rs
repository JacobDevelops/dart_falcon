//! Reports function bodies with too many statements.
//!
//! `max_statements` (integer, default: 20) limits executable and declaration
//! statements, excluding blocks, labels, empty statements, and local function
//! declarations. Nested callable bodies are measured independently.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MaxStatements;
impl Rule for MaxStatements {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            threshold: crate::lint::options::usize(
                crate::lint::options::for_rule(ctx, "max-statements"),
                "max_statements",
            )
            .unwrap_or(20),
            counts: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    threshold: usize,
    counts: Vec<usize>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_function_body(&mut self, _node: FunctionBody<'a>) -> StructuralTraversal {
        self.counts.push(0); StructuralTraversal::Continue
    }
    fn exit_function_body(&mut self, node: FunctionBody<'a>) {
        if let Some(count) = self.counts.pop() && count > self.threshold {
            self.diags.push(self.ctx.diagnostic("max-statements", format!("Function contains {count} statements (max {}). Extract smaller functions.", self.threshold), node.span()));
        }
    }
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if !matches!(node, Stmt::Block(_) | Stmt::Empty(_) | Stmt::Labeled(_) | Stmt::LocalFunc(_) | Stmt::Error(_))
            && let Some(count) = self.counts.last_mut() { *count += 1; }
        StructuralTraversal::Continue
    }
}
}
