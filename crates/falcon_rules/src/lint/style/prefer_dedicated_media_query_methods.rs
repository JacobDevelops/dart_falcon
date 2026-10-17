//! Flags `MediaQuery.of(context).size` access in favor of `MediaQuery.sizeOf`.
//!
//! Reading `.width` or `.height` off `MediaQuery.of(context).size` subscribes the
//! widget to the entire `MediaQueryData`, so it rebuilds whenever any media property
//! changes — text scale, padding, orientation — not just the screen size. Flutter's
//! dedicated `MediaQuery.sizeOf(context)` aspect getter subscribes only to size
//! changes, avoiding those spurious rebuilds. Use `MediaQuery.sizeOf(context)` (and
//! its siblings) instead of pulling fields out of the whole `MediaQueryData`.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferDedicatedMediaQueryMethods;

impl Rule for PreferDedicatedMediaQueryMethods {
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
fn check_media_query_size_field(
    object: &Expr,
    field: &Identifier,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if (field.name == "width" || field.name == "height") && is_media_query_size(object) {
        diags.push(ctx.diagnostic(
            "prefer-dedicated-media-query-methods",
            "Use MediaQuery.sizeOf(context) instead of MediaQuery.of(context).size.",
            *span,
        ));
    }
}

fn is_media_query_size(expr: &Expr) -> bool {
    if let Expr::Field { object, field, .. } = expr.unparenthesized()
        && field.name == "size"
        && let Expr::Call { callee, args, .. } = object.unparenthesized()
        && let Expr::Field {
            object: mq_obj,
            field: method,
            ..
        } = callee.unparenthesized()
        && let Expr::Ident(ident) = mq_obj.unparenthesized()
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

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Field {
            object,
            field,
            span,
            ..
        } = node
        {
            check_media_query_size_field(object, field, span, &mut self.diags, self.ctx);
        }
        StructuralTraversal::Continue
    }
}
}
