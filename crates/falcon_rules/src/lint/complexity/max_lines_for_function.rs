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
    #[serde(deserialize_with = "falcon_config::integer")]
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
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
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

fn max_lines_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::get::<Options>(ctx, MaxLinesForFunction::NAME).max_lines
}

fn check_function_lines(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let source = ctx.source().as_bytes();
    let lines = source[span.start.min(source.len())..span.end.min(source.len())]
        .split(|&byte| byte == b'\n')
        .count();
    let threshold = max_lines_option(ctx);

    if lines > threshold {
        diags.push(ctx.diagnostic(
            format!("Function exceeds the maximum number of lines ({threshold})."),
            DiagSpan {
                start: span.start,
                end: span.start,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_limit_counts_only_the_span_with_lf_or_crlf() {
        for newline in ["\n", "\r\n"] {
            for (newlines, expected) in [(99, 0), (100, 1)] {
                let prefix = format!("// 😀{newline}").repeat(150);
                let source = format!("{prefix}void f() {{{}}}", newline.repeat(newlines));
                let (parsed, errors) =
                    falcon_analyze::ParsedFile::parse_for_package(source.as_str(), None);
                assert!(errors.is_empty(), "{errors:?}");
                let parsed = std::sync::Arc::new(parsed);
                let config = falcon_config::FalconConfig::default();
                let ctx = AnalyzeContext::new(std::path::Path::new("test.dart"), &config, &parsed);
                let span = Span {
                    start: prefix.len(),
                    end: source.len(),
                };
                let mut diagnostics = Vec::new();
                check_function_lines(&span, &mut diagnostics, &ctx);
                assert_eq!(diagnostics.len(), expected, "{newline:?}, {newlines}");
                if let Some(diagnostic) = diagnostics.first() {
                    assert_eq!(diagnostic.span.start, prefix.len());
                    assert_eq!(diagnostic.span.end, prefix.len());
                }
            }
        }
    }
}
