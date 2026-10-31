//! Flags non-ASCII characters written literally in strings, including raw and
//! triple-quoted strings. Each adjacent literal is checked independently.
//!
//! Unicode escapes are accepted. Interpolation expressions are excluded, while
//! nested string literals are checked on their own.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Program, StringInterpolation, StringLitNode};

#[derive(Default)]
pub struct AvoidNonAsciiSymbols;

impl Rule for AvoidNonAsciiSymbols {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) -> StructuralTraversal {
        let mut interpolations = node.interpolations().map(StringInterpolation::span).peekable();
        for (lit, span) in node.segments() {
            if !lit.terminated || lit.content.is_ascii() {
                continue;
            }
            let mut start = span.start + lit.content_offset;
            let end = start + lit.content.len();
            let mut non_ascii = false;
            while let Some(expression) =
                interpolations.next_if(|expression| expression.start < end)
            {
                if expression.start < start || expression.end > end {
                    continue;
                }
                if !self.ctx.source()[start..expression.start].is_ascii() {
                    non_ascii = true;
                    break;
                }
                start = expression.end;
            }
            if non_ascii || !self.ctx.source()[start..end].is_ascii() {
                self.diags.push(self.ctx.diagnostic(
                    "Replace non-ASCII characters in this string with escapes or move it to localized resources.",
                    span,
                ));
            }
        }
        StructuralTraversal::Continue
    }
}
}
