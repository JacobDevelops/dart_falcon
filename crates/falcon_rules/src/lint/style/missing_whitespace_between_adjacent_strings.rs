//! Flags adjacent string literals that join two words without a space between
//! them.
//!
//! When a part of an adjacent-string concatenation contains whitespace (so it
//! reads as prose), neither it nor the next part supplying whitespace at the
//! seam usually means a missing space:
//!
//! ```dart
//! var s = 'This string will be concatenated'  // flagged
//!     'with the next one.';
//! var s = 'This string will be concatenated '
//!     'with the next one.';                   // fine
//! ```
//!
//! An interpolation at the seam counts as whitespace. Strings passed straight
//! to `RegExp(...)` or an unqualified `matches(...)` call are skipped, since
//! patterns are split for readability rather than prose.
//!
//! Adopted from the official `missing_whitespace_between_adjacent_strings`
//! lint.

use crate::lint::string_lit::{LiteralPart, decoded_text, literal_parts};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MissingWhitespaceBetweenAdjacentStrings;

impl Rule for MissingWhitespaceBetweenAdjacentStrings {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            exempt: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

const WHITESPACE: [char; 4] = [' ', '\n', '\r', '\t'];

/// Whitespace facts about one part of an adjacent-string concatenation.
struct Seam {
    starts_with_whitespace: bool,
    ends_with_whitespace: bool,
    has_whitespace: bool,
}

impl Seam {
    fn of(lit: &StringDelimiter<'_>) -> Self {
        let parts = literal_parts(lit);
        let text = decoded_text(lit, &parts);
        let is_interpolation =
            |part: Option<&LiteralPart<'_>>| matches!(part, Some(LiteralPart::Interpolation(_)));
        Seam {
            starts_with_whitespace: is_interpolation(parts.first()) || text.starts_with(WHITESPACE),
            ends_with_whitespace: is_interpolation(parts.last()) || text.ends_with(WHITESPACE),
            has_whitespace: text.contains(WHITESPACE),
        }
    }
}

/// Whether a call passes its positional arguments as a regular expression.
fn is_pattern_call<'a>(node: Expr<'a>) -> Option<ArgList<'a>> {
    match node.unparenthesized() {
        Expr::Call(call) => match call.callee().unparenthesized() {
            Expr::Ident(name) if matches!(name.name(), "RegExp" | "matches") => Some(call.args()),
            Expr::Field(field)
                if field.field().name() == "RegExp"
                    && matches!(field.object().unparenthesized(), Expr::Ident(_)) =>
            {
                Some(call.args())
            }
            _ => None,
        },
        Expr::New(expr)
            if matches!(expr.dart_type(), DartType::Named(named)
            if named.segments().last().is_some_and(|segment| segment.name() == "RegExp")) =>
        {
            Some(expr.args())
        }
        _ => None,
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Spans of adjacent strings passed positionally to a pattern call.
    exempt: Vec<Span>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::Paren(_)) {
            return StructuralTraversal::Continue;
        }
        if let Some(args) = is_pattern_call(node) {
            self.exempt.extend(args.positional().filter_map(
                |arg| match arg.unparenthesized() {
                    Expr::StringLit(string) if string.is_adjacent() => Some(string.span()),
                    _ => None,
                },
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) -> StructuralTraversal {
        if !node.is_adjacent() || self.exempt.contains(&node.span()) {
            return StructuralTraversal::SkipChildren;
        }
        let seams: Vec<(Seam, Span)> = node
            .segments()
            .map(|(lit, span)| (Seam::of(&lit), span))
            .collect();
        for pair in seams.windows(2) {
            let [(current, span), (next, _)] = pair else {
                continue;
            };
            if current.ends_with_whitespace
                || next.starts_with_whitespace
                || !current.has_whitespace
            {
                continue;
            }
            self.diags.push(self.ctx.diagnostic(
                "Add whitespace between these adjacent strings.",
                *span,
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}
