//! Flags `x ?? null` and `null ?? x`, where the `null` operand is redundant.
//!
//! The `??` operator falls back to its right operand only when the left is null,
//! so `x ?? null` always evaluates to `x` and `null ?? x` always evaluates to
//! `x`. The literal `null` contributes nothing and typically marks an unfinished
//! edit or a misunderstanding of the operator. Delete the `null` operand, and the
//! `??` along with it.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UnnecessaryNullInIfNullOperators;

impl Rule for UnnecessaryNullInIfNullOperators {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Binary {
            op: BinaryOp::NullCoalesce,
            left,
            right,
            span,
            ..
        } = node
            && (matches!(left.unparenthesized(), Expr::NullLit { .. })
                || matches!(right.unparenthesized(), Expr::NullLit { .. }))
        {
            self.diags.push(self.ctx.diagnostic(
                "unnecessary-null-in-if-null-operators",
                "Unnecessary null in an if-null ('??') operator.",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}
