//! Flags string literals whose only escapes are `\\` and `\$`, which a raw
//! string would express without any backslash.
//!
//! ```dart
//! var s = 'A string with only \\ and \$'; // flagged
//! var s = r'A string with only \ and $';  // preferred
//! var s = 'tab\there \$';                 // fine: `\t` needs a regular string
//! ```
//!
//! Interpolated literals are not checked, since a raw string cannot
//! interpolate. Each part of an adjacent-string concatenation is checked on
//! its own.
//!
//! Adopted from the official `use_raw_strings` lint.

use crate::lint::string_lit::{has_interpolation, literal_parts};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

#[derive(Default)]
pub struct UseRawStrings;

impl Rule for UseRawStrings {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

/// Whether the body has at least one escape and every escape is `\\` or `\$`.
fn only_raw_expressible_escapes(content: &str) -> bool {
    let bytes = content.as_bytes();
    let mut has_escape = false;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' {
            has_escape = true;
            i += 1;
            if !matches!(bytes[i], b'\\' | b'$') {
                return false;
            }
        }
        i += 1;
    }
    has_escape
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) {
        for (lit, span) in node.segments() {
            if !lit.terminated
                || lit.is_raw
                || has_interpolation(&literal_parts(&lit))
                || !only_raw_expressible_escapes(lit.content)
            {
                continue;
            }
            self.diags.push(self.ctx.diagnostic(
                "use-raw-strings",
                "Use a raw string to avoid using escapes.",
                span,
            ));
        }
    }
}
