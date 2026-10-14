//! Flags a multiline string spanning several lines whose text does not start
//! with a line break right after the opening quotes.
//!
//! Starting the text on its own line keeps every line aligned at the same
//! indentation, and Dart drops that first line break from the value:
//!
//! ```dart
//! var s1 = '''{
//!   "a": 1
//! }''';      // flagged
//! var s2 = '''
//! {
//!   "a": 1
//! }''';      // preferred
//! ```
//!
//! Triple-quoted literals that stay on one line are not flagged.
//!
//! Adopted from the official `leading_newlines_in_multiline_strings` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct LeadingNewlinesInMultilineStrings;

impl Rule for LeadingNewlinesInMultilineStrings {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in node.segments() {
            if !lit.triple || lit.content.starts_with(['\n', '\r']) {
                continue;
            }
            let spans_lines = self
                .ctx
                .source
                .get(span.start..span.end)
                .is_some_and(|text| text.contains(['\n', '\r']));
            if spans_lines {
                self.diags.push(self.ctx.diagnostic(
                    "leading-newlines-in-multiline-strings",
                    "Missing a newline at the beginning of a multiline string.",
                    span,
                ));
            }
        }
    }
}
