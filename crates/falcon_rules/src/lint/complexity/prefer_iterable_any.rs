//! Flags `iterable.where(...).isNotEmpty`.
//!
//! Testing whether a filtered iterable is non-empty walks the filter just to
//! check for any match; `iterable.any(...)` short-circuits on the first match
//! and reads better. The rule matches an `.isNotEmpty` access on the result of a
//! `.where(...)` call.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferIterableAny;

impl Rule for PreferIterableAny {
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

fn is_where_is_not_empty(expr: Expr<'_>) -> Option<Span> {
    // Match: something.where(...).isNotEmpty
    if let Expr::Field(field) = expr.unparenthesized()
        && field.field().name() == "isNotEmpty"
        && let Expr::Call(call) = field.object().unparenthesized()
        && let Expr::Field(where_field) = call.callee().unparenthesized()
        && where_field.field().name() == "where"
    {
        return Some(field.span());
    }
    None
}

/// Detection runs on every expression the shared walker reaches. The walker is
/// exhaustive over the syntax tree, so a violation cannot hide inside newer syntax.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::Paren(_)) {
            return StructuralTraversal::Continue;
        }
        if let Some(span) = is_where_is_not_empty(node) {
            self.diags.push(self.ctx.diagnostic(
                "Use .any() instead of .where().isNotEmpty.",
                span,
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
