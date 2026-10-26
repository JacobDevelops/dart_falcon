use crate::lint::string_lit::{LiteralPart, decoded_text, literal_parts};
use falcon_analyze::{
    AnalyzeContext, StructuralRuleVisitor, StructuralTraversal, structural_visitor,
};
use falcon_diagnostics::{Diagnostic, FixKind, TextEdit};
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
            let diagnostic = self.ctx.diagnostic(self.preference.message, span);
            let width = if lit.triple { 3 } else { 1 };
            let opening = span.start + usize::from(lit.is_raw);
            let closing = span.end - width;
            let delimiter = match (preferred, lit.triple) {
                (b'\'', false) => "'",
                (b'\'', true) => "'''",
                (_, false) => "\"",
                (_, true) => "\"\"\"",
            };
            // Adjacent quote runs can otherwise become a triple delimiter.
            let bytes = self.ctx.source().as_bytes();
            let mut opening_edit = TextEdit::new(
                falcon_diagnostics::Span {
                    start: opening,
                    end: opening + width,
                },
                delimiter,
            );
            if !lit.is_raw
                && span.start.checked_sub(1).and_then(|index| bytes.get(index)) == Some(&preferred)
            {
                opening_edit.replacement.insert(0, ' ');
            }
            let mut closing_edit = TextEdit::new(
                falcon_diagnostics::Span {
                    start: closing,
                    end: span.end,
                },
                delimiter,
            );
            if bytes.get(span.end) == Some(&preferred) {
                closing_edit.replacement.push(' ');
            }
            self.diags.push(self.ctx.with_fix(
                diagnostic,
                "Use the preferred quote style",
                FixKind::Safe,
                vec![opening_edit, closing_edit],
            ));
        }
        StructuralTraversal::Continue
    }
}
}
