//! Flags raw string literals that contain no backslash or `$`.
//!
//! A raw string only differs from a regular one when its text holds `\` or `$`;
//! otherwise the `r` prefix is noise:
//!
//! ```dart
//! var s1 = r'a b c';  // flagged
//! var s2 = 'a b c';   // preferred
//! var s3 = r'a \ b';  // fine: the backslash stays literal
//! ```
//!
//! Each part of an adjacent-string concatenation is checked on its own.
//!
//! Adopted from the official `unnecessary_raw_strings` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UnnecessaryRawStrings;

impl Rule for UnnecessaryRawStrings {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
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
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_string_lit(&mut self, node: &StringLitNode) -> StructuralTraversal {
        for (lit, span) in node.segments() {
            if lit.terminated && lit.is_raw && !lit.content.contains(['\\', '$']) {
                self.diags.push(self.ctx.diagnostic(
                    "unnecessary-raw-strings",
                    "Unnecessary use of a raw string.",
                    span,
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
