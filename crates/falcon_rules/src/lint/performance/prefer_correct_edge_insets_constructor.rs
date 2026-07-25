//! Flags an `EdgeInsets.only(...)` that a more specific constructor expresses better.
//!
//! `EdgeInsets.only` with symmetric values is clearer and cheaper as
//! `EdgeInsets.all(n)` (all four sides equal), `EdgeInsets.symmetric(vertical:
//! n)` (only top and bottom, equal), or `EdgeInsets.symmetric(horizontal: n)`
//! (only left and right, equal). The rule detects these three shapes on
//! `EdgeInsets.only(...)` (and a direct `EdgeInsets(...)` carrying the same
//! named arguments) and reports the equivalent to prefer.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferCorrectEdgeInsetsConstructor;

impl Rule for PreferCorrectEdgeInsetsConstructor {
    fn name(&self) -> &'static str {
        "prefer-correct-edge-insets-constructor"
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
fn check_edge_insets_only_call(
    callee: &Expr,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Expr::Field { object, field, .. } = callee
        && let Expr::Ident(ident) = object.as_ref()
        && ident.name == "EdgeInsets"
        && field.name == "only"
    {
        let should_flag = should_use_better_constructor(args);
        if should_flag {
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
                "prefer-correct-edge-insets-constructor",
                Severity::Warning,
                "EdgeInsets.only() should use EdgeInsets.symmetric() or EdgeInsets.all().",
                ctx.file_path.to_string_lossy().into_owned(),
                report_span,
            ));
        }
    }
}

fn check_edge_insets_only(
    dart_type: &DartType,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let DartType::Named(nt) = dart_type
        && nt.segments.len() == 1
        && nt.segments[0].name == "EdgeInsets"
    {
        let should_flag = should_use_better_constructor(args);
        if should_flag {
            diags.push(Diagnostic::new(
                "prefer-correct-edge-insets-constructor",
                Severity::Warning,
                "EdgeInsets.only() should use EdgeInsets.symmetric() or EdgeInsets.all().",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

fn should_use_better_constructor(args: &ArgList) -> bool {
    let mut left = None;
    let mut right = None;
    let mut top = None;
    let mut bottom = None;

    for named in &args.named {
        let value_str = expr_to_string(&named.value);
        match named.name.name.as_str() {
            "left" => left = value_str,
            "right" => right = value_str,
            "top" => top = value_str,
            "bottom" => bottom = value_str,
            _ => {}
        }
    }

    if let (Some(l), Some(r), Some(t), Some(b)) =
        (left.clone(), right.clone(), top.clone(), bottom.clone())
        && l == r
        && r == t
        && t == b
    {
        return true;
    }

    if left.is_none()
        && right.is_none()
        && let (Some(t), Some(b)) = (top.clone(), bottom.clone())
        && t == b
    {
        return true;
    }

    if top.is_none()
        && bottom.is_none()
        && let (Some(l), Some(r)) = (left.clone(), right.clone())
        && l == r
    {
        return true;
    }

    false
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
        match node {
            Expr::New {
                dart_type,
                args,
                span,
                ..
            } => check_edge_insets_only(dart_type, args, span, &mut self.diags, self.ctx),
            Expr::Call {
                callee, args, span, ..
            } => check_edge_insets_only_call(callee, args, span, &mut self.diags, self.ctx),
            _ => {}
        }
        walk_expr(self, node);
    }
}
