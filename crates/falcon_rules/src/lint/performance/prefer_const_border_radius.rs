//! Flags `BorderRadius.only(...)` whose four corner radii are all equal.
//!
//! When every corner uses the same `Radius.circular(n)`,
//! `BorderRadius.all(Radius.circular(n))` preserves const construction and
//! avoids repeating the radius. Unlike `BorderRadius.circular`, `all` is const.
//! All four corners must be present and their circular values textually equal.

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
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}
fn check_border_radius_only_call(
    args: ArgList<'_>,
    span: Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if all_border_radii_equal(args)
    {
        let source = ctx.source();
        let start_line = source[..span.start].chars().filter(|&c| c == '\n').count();
        let end_line = source[..span.end].chars().filter(|&c| c == '\n').count();

        let report_span = if start_line == end_line {
            span
        } else {
            let opening_line_end = source[span.start..]
                .find('\n')
                .map_or(source.len(), |off| span.start + off);
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

        diags.push(ctx.diagnostic("`BorderRadius.only()` with all equal radii should use `BorderRadius.all(Radius.circular(...))`.", report_span));
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

fn named_construction<'a>(expr: Expr<'a>, class: &str, constructor: &str) -> Option<ArgList<'a>> {
    match expr.unparenthesized() {
        Expr::Call(call) => {
            let Expr::Field(field) = call.callee().unparenthesized() else { return None; };
            let Expr::Ident(ident) = field.object().unparenthesized() else { return None; };
            (ident.name() == class && field.field().name() == constructor).then(|| call.args())
        }
        Expr::New(new) => {
            let DartType::Named(named) = new.dart_type() else { return None; };
            let mut segments = named.segments();
            let name = new.constructor_name().or_else(|| segments.next_back())?;
            (segments.next_back()?.name() == class && name.name() == constructor).then(|| new.args())
        }
        _ => None,
    }
}

fn extract_radius_value(expr: Expr<'_>) -> Option<&str> {
    let args = named_construction(expr, "Radius", "circular")?;
    let mut arguments = args.positional();
    let value = scalar_text(arguments.next()?)?;
    arguments.next().is_none().then_some(value)
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
        if let Some(args) = named_construction(node, "BorderRadius", "only") {
            check_border_radius_only_call(args, node.span(), &mut self.diags, self.ctx);
        }
        StructuralTraversal::Continue
    }
}
}
