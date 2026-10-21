//! Flags a function or method longer than the configured line limit.
//!
//! Long functions are hard to reason about and test; extracting helpers keeps
//! each unit focused. The rule measures a function's span from its declaration
//! through the end of its body (in source lines) and reports at the declaration
//! when the length exceeds the threshold. Operator bodies are included, while
//! abstract/external declarations and nested functions or closures are not measured.
//!
//! ## Options
//!
//! `max-lines` (integer, default: 100) — flag functions/methods spanning more
//! than this many lines.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum number of source lines allowed in a function.
    #[serde(deserialize_with = "crate::lint::options::integer")]
    max_lines: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_lines: 100 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct MaxLinesForFunction;

impl Rule for MaxLinesForFunction {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if node.body().is_some() {
            check_function_lines(&node.span(), &mut self.diags, self.ctx);
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let Some((_, span)) = falcon_syntax::cst::visit::declared_member_body(node) {
            check_function_lines(&span, &mut self.diags, self.ctx);
        }
        StructuralTraversal::SkipChildren
    }
}
}

use crate::lint::expressions::line_of;

fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::get::<Options>(ctx, MaxLinesForFunction::NAME).max_lines
}

fn check_function_lines(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let start_line = line_of(ctx.source(), span.start);
    let end_line = line_of(ctx.source(), span.end);
    let lines = end_line - start_line + 1;
    let threshold = max_lines_option(ctx);

    if lines > threshold {
        diags.push(ctx.diagnostic(format!("Function exceeds the maximum number of lines ({threshold})."), DiagSpan {
            start: span.start,
            end: span.start,
        }));
    }
}
