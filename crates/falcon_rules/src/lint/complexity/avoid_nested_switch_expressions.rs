//! Reports deeply nested switch expressions.
//!
//! `max-nesting` (integer, default: 1) limits nesting of this construct.
//! Local functions and closures restart the nesting count.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum number of nested switch expressions.
    #[serde(deserialize_with = "crate::lint::options::integer")]
    max_nesting: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_nesting: 1 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct AvoidNestedSwitchExpressions;
impl Rule for AvoidNestedSwitchExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            threshold: crate::lint::options::get::<Options>(ctx, Self::NAME).max_nesting,
            depth: 0,
            parents: Vec::new(),
            bodies: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    threshold: usize,
    depth: usize,
    parents: Vec<usize>,
    bodies: Vec<usize>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_function_body(&mut self, _node: FunctionBody<'a>) -> StructuralTraversal {
        self.bodies.push(self.depth); self.depth = 0; StructuralTraversal::Continue
    }
    fn exit_function_body(&mut self, _node: FunctionBody<'a>) { self.depth = self.bodies.pop().unwrap_or(0); }
fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.parents.push(self.depth);
        if matches!(node, Expr::Switch(_)) {
            self.depth += 1;
            if self.depth == self.threshold.saturating_add(1) {
                self.diags.push(self.ctx.diagnostic(format!("Nesting exceeds {} levels. Extract the nested construct into a helper.", self.threshold), node.span()));
            }
        }
        StructuralTraversal::Continue
    }
    fn exit_expr(&mut self, _node: Expr<'a>) { self.depth = self.parents.pop().unwrap_or(0); }
}
}
