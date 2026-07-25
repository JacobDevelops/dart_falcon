//! Flags `MediaQuery.of(context).size` access in favor of `MediaQuery.sizeOf`.
//!
//! Reading `.width` or `.height` off `MediaQuery.of(context).size` subscribes the
//! widget to the entire `MediaQueryData`, so it rebuilds whenever any media property
//! changes — text scale, padding, orientation — not just the screen size. Flutter's
//! dedicated `MediaQuery.sizeOf(context)` aspect getter subscribes only to size
//! changes, avoiding those spurious rebuilds. Use `MediaQuery.sizeOf(context)` (and
//! its siblings) instead of pulling fields out of the whole `MediaQueryData`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferDedicatedMediaQueryMethods;

impl Rule for PreferDedicatedMediaQueryMethods {
    fn name(&self) -> &'static str {
        "prefer-dedicated-media-query-methods"
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
fn check_media_query_size_field(
    object: &Expr,
    field: &Identifier,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if (field.name == "width" || field.name == "height") && is_media_query_size(object) {
        diags.push(Diagnostic::new(
            "prefer-dedicated-media-query-methods",
            Severity::Warning,
            "Use MediaQuery.sizeOf(context) instead of MediaQuery.of(context).size.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn is_media_query_size(expr: &Expr) -> bool {
    if let Expr::Field { object, field, .. } = expr
        && field.name == "size"
        && let Expr::Call { callee, args, .. } = object.as_ref()
        && let Expr::Field {
            object: mq_obj,
            field: method,
            ..
        } = callee.as_ref()
        && let Expr::Ident(ident) = mq_obj.as_ref()
    {
        return ident.name == "MediaQuery" && method.name == "of" && args.positional.len() == 1;
    }
    false
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
        if let Expr::Field {
            object, field, span, ..
        } = node
        {
            check_media_query_size_field(object, field, span, &mut self.diags, self.ctx);
        }
        walk_expr(self, node);
    }
}
