//! Flags an `EdgeInsets.only(...)` that a more specific constructor expresses better.
//!
//! `EdgeInsets.only` with symmetric values is clearer and cheaper as
//! `EdgeInsets.all(n)` (all four sides equal), `EdgeInsets.symmetric(vertical:
//! n)` (only top and bottom, equal), or `EdgeInsets.symmetric(horizontal: n)`
//! (only left and right, equal). The rule detects these three shapes on
//! `EdgeInsets.only(...)` (and a direct `EdgeInsets(...)` carrying the same
//! named arguments) and reports the equivalent to prefer.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferCorrectEdgeInsetsConstructor;

impl Rule for PreferCorrectEdgeInsetsConstructor {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}
fn check_edge_insets_only_call(
    callee: Expr<'_>,
    args: ArgList<'_>,
    span: Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Expr::Field(field) = callee.unparenthesized()
        && let Expr::Ident(ident) = field.object().unparenthesized()
        && ident.name() == "EdgeInsets"
        && field.field().name() == "only"
    {
        let should_flag = should_use_better_constructor(args);
        if should_flag {
            let source = ctx.source();
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

            diags.push(ctx.diagnostic("`EdgeInsets.only()` should use `EdgeInsets.symmetric()` or `EdgeInsets.all()`.", report_span));
        }
    }
}

fn check_edge_insets_only(
    dart_type: DartType<'_>,
    args: ArgList<'_>,
    span: Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let DartType::Named(nt) = dart_type
        && let (Some(segment), None) = {
            let mut segments = nt.segments();
            (segments.next(), segments.next())
        }
        && segment.name() == "EdgeInsets"
    {
        let should_flag = should_use_better_constructor(args);
        if should_flag {
            diags.push(ctx.diagnostic("`EdgeInsets.only()` should use `EdgeInsets.symmetric()` or `EdgeInsets.all()`.", span));
        }
    }
}

fn should_use_better_constructor(args: ArgList<'_>) -> bool {
    let mut left = None;
    let mut right = None;
    let mut top = None;
    let mut bottom = None;

    for named in args.named() {
        let value_str = scalar_text(named.value());
        match named.name().name() {
            "left" => left = value_str,
            "right" => right = value_str,
            "top" => top = value_str,
            "bottom" => bottom = value_str,
            _ => {}
        }
    }

    if let (Some(l), Some(r), Some(t), Some(b)) = (left, right, top, bottom)
        && l == r
        && r == t
        && t == b
    {
        return true;
    }

    if left.is_none()
        && right.is_none()
        && let (Some(t), Some(b)) = (top, bottom)
        && t == b
    {
        return true;
    }

    if top.is_none()
        && bottom.is_none()
        && let (Some(l), Some(r)) = (left, right)
        && l == r
    {
        return true;
    }

    false
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
        match node {
            Expr::New(construction) => check_edge_insets_only(construction.dart_type(), construction.args(), construction.span(), &mut self.diags, self.ctx),
            Expr::Call(call) => check_edge_insets_only_call(call.callee(), call.args(), call.span(), &mut self.diags, self.ctx),
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}
