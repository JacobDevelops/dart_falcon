//! Flags `BorderRadius.only(...)` whose four corner radii are all equal.
//!
//! When every corner uses the same `Radius.circular(n)`,
//! `BorderRadius.circular(n)` is shorter and const-constructible, so Flutter can
//! canonicalize and reuse the value instead of rebuilding it. The rule fires
//! only when all four corners (`topLeft`, `topRight`, `bottomLeft`,
//! `bottomRight`) are present and their `Radius.circular(...)` values are
//! textually equal.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferConstBorderRadius;

impl Rule for PreferConstBorderRadius {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}
fn check_border_radius_only_call(
    callee: Expr<'_>,
    args: ArgList<'_>,
    span: Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Expr::Field(field) = callee.unparenthesized()
        && let Expr::Ident(ident) = field.object().unparenthesized()
        && ident.name() == "BorderRadius"
        && field.field().name() == "only"
        && all_border_radii_equal(args)
    {
        let source = ctx.source;
        let start_line = source[..span.start].chars().filter(|&c| c == '\n').count();
        let end_line = source[..span.end].chars().filter(|&c| c == '\n').count();

        let report_span = if start_line == end_line {
            span
        } else {
            let opening_line_end = source[span.start..]
                .find('\n')
                .map(|off| span.start + off)
                .unwrap_or(source.len());
            let opening_line_text = &source[span.start..opening_line_end];

            if opening_line_text.contains("*/") {
                Span {
                    start: span.start,
                    end: span.start + 1,
                }
            } else {
                Span {
                    start: span.end - 1,
                    end: span.end,
                }
            }
        };

        diags.push(ctx.diagnostic(
            "prefer-const-border-radius",
            "BorderRadius.only() with all equal radii should use BorderRadius.circular().",
            report_span,
        ));
    }
}

fn all_border_radii_equal(args: ArgList<'_>) -> bool {
    let mut top_left = None;
    let mut top_right = None;
    let mut bottom_left = None;
    let mut bottom_right = None;

    for named in args.named() {
        let radius_value = extract_radius_value(named.value());
        match named.name().name() {
            "topLeft" => top_left = radius_value,
            "topRight" => top_right = radius_value,
            "bottomLeft" => bottom_left = radius_value,
            "bottomRight" => bottom_right = radius_value,
            _ => {}
        }
    }

    if let (Some(tl), Some(tr), Some(bl), Some(br)) =
        (top_left, top_right, bottom_left, bottom_right)
    {
        tl == tr && tr == bl && bl == br
    } else {
        false
    }
}

fn extract_radius_value(expr: Expr<'_>) -> Option<&str> {
    if let Expr::Call(call) = expr.unparenthesized()
        && let Expr::Field(field) = call.callee().unparenthesized()
        && let Expr::Ident(ident) = field.object().unparenthesized()
        && ident.name() == "Radius"
        && field.field().name() == "circular"
        && call.args().positional().len() == 1
    {
        return scalar_text(call.args().positional().next().unwrap());
    }
    None
}

use crate::lint::expressions::scalar_text;

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Call(call) = node {
            check_border_radius_only_call(call.callee(), call.args(), call.span(), &mut self.diags, self.ctx);
        }
        StructuralTraversal::Continue
    }
}
}
