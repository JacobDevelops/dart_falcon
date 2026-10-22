use crate::lint::string_lit::{LiteralPart, decoded_text, literal_parts};
use falcon_analyze::{
    AnalyzeContext, StructuralRuleVisitor, StructuralTraversal, structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

/// The quote style a rule enforces; shared with `prefer-double-quotes`.
pub(crate) struct QuotePreference {
    pub(crate) message: &'static str,
    pub(crate) preferred: u8,
}

pub(crate) struct QuoteVisitor<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    preference: QuotePreference,
    /// How many interpolated expressions enclose the node being visited.
    interpolation_depth: usize,
}

impl<'a> QuoteVisitor<'a> {
    pub(crate) fn new(ctx: &'a AnalyzeContext<'a>, preference: QuotePreference) -> Self {
        QuoteVisitor {
            diags: Vec::new(),
            ctx,
            preference,
            interpolation_depth: 0,
        }
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for QuoteVisitor<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_string_interpolation(&mut self, _node: StringInterpolation<'a>) -> StructuralTraversal {
        self.interpolation_depth += 1;
        StructuralTraversal::Continue
    }

    fn exit_string_interpolation(&mut self, _node: StringInterpolation<'a>) {
        self.interpolation_depth -= 1;
    }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) -> StructuralTraversal {
        if self.interpolation_depth > 0 {
            return StructuralTraversal::Continue;
        }
        let preferred = self.preference.preferred;
        let preferred_char = char::from(preferred);
        for (lit, span) in node.segments() {
            if !lit.terminated || lit.quote == preferred {
                continue;
            }
            let parts = literal_parts(&lit);
            if decoded_text(&lit, &parts).contains(preferred_char) {
                continue;
            }
            // Bail out on "strings ${x ? 'containing' : 'other'} strings".
            if parts.iter().any(|part| {
                matches!(part, LiteralPart::Interpolation(source) if source.contains(['\'', '"']))
            }) {
                continue;
            }
            self.diags.push(self.ctx.diagnostic(
                self.preference.message,
                span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
