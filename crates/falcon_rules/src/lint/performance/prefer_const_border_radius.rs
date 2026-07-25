//! Flags `BorderRadius.only(...)` whose four corner radii are all equal.
//!
//! When every corner uses the same `Radius.circular(n)`,
//! `BorderRadius.circular(n)` is shorter and const-constructible, so Flutter can
//! canonicalize and reuse the value instead of rebuilding it. The rule fires
//! only when all four corners (`topLeft`, `topRight`, `bottomLeft`,
//! `bottomRight`) are present and their `Radius.circular(...)` values are
//! textually equal.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferConstBorderRadius;

impl Rule for PreferConstBorderRadius {
    fn name(&self) -> &'static str {
        "prefer-const-border-radius"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}
fn check_border_radius_only_call(
    callee: &Expr,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Expr::Field { object, field, .. } = callee
        && let Expr::Ident(ident) = object.as_ref()
        && ident.name == "BorderRadius"
        && field.name == "only"
        && all_border_radii_equal(args)
    {
        let source = ctx.source;
        let start_line = source[..span.start].chars().filter(|&c| c == '\n').count();
        let end_line = source[..span.end].chars().filter(|&c| c == '\n').count();

        let report_span = if start_line == end_line {
            DiagSpan {
                start: span.start,
                end: span.end,
            }
        } else {
            let opening_line_end = source[span.start..]
                .find('\n')
                .map(|off| span.start + off)
                .unwrap_or(source.len());
            let opening_line_text = &source[span.start..opening_line_end];

            if opening_line_text.contains("*/") {
                DiagSpan {
                    start: span.start,
                    end: span.start + 1,
                }
            } else {
                DiagSpan {
                    start: span.end - 1,
                    end: span.end,
                }
            }
        };

        diags.push(Diagnostic::new(
            "prefer-const-border-radius",
            Severity::Warning,
            "BorderRadius.only() with all equal radii should use BorderRadius.circular().",
            ctx.file_path.to_string_lossy().into_owned(),
            report_span,
        ));
    }
}

fn all_border_radii_equal(args: &ArgList) -> bool {
    let mut top_left = None;
    let mut top_right = None;
    let mut bottom_left = None;
    let mut bottom_right = None;

    for named in &args.named {
        let radius_value = extract_radius_value(&named.value);
        match named.name.name.as_str() {
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

fn extract_radius_value(expr: &Expr) -> Option<String> {
    if let Expr::Call { callee, args, .. } = expr
        && let Expr::Field { object, field, .. } = callee.as_ref()
        && let Expr::Ident(ident) = object.as_ref()
        && ident.name == "Radius"
        && field.name == "circular"
        && args.positional.len() == 1
    {
        return expr_to_string(&args.positional[0]);
    }
    None
}

fn expr_to_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::IntLit { value, .. } => Some(value.clone()),
        Expr::DoubleLit { value, .. } => Some(value.clone()),
        Expr::Ident(id) => Some(id.name.clone()),
        _ => None,
    }
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call {
            callee, args, span, ..
        } = node
        {
            check_border_radius_only_call(callee, args, span, &mut self.diags, self.ctx);
        }
        walk_expr(self, node);
    }
}
