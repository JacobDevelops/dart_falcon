//! Flags a negated type test `!(x is T)` that should use the `is!` operator.
//!
//! Dart provides the `is!` operator precisely so negated type checks read
//! directly as `x is! T` instead of wrapping the test in parentheses and negating
//! it, which is easier to misread and to accidentally mis-parenthesize. The rule
//! fires on a logical-not applied to a non-negated `is` expression; a test that
//! already uses `is!` produces no negation to flag.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct PreferIsNotOperator;

impl Rule for PreferIsNotOperator {
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
        if let Expr::Unary {
            op: UnaryOp::Bang,
            operand,
            span,
        } = node
            && matches!(operand.as_ref(), Expr::Is { negated: false, .. })
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-is-not-operator",
                "Prefer using the 'is!' operator.",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}
