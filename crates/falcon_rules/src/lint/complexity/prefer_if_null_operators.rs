//! Flags `x == null ? y : x` (and the inverted `x != null ? x : y`).
//!
//! A ternary that returns its own operand on the non-null branch is exactly the
//! `??` if-null operator: `x ?? y`. The operator is shorter and evaluates `x`
//! only once. The rule takes the operand of the `== null`/`!= null` check and
//! compares it (ignoring whitespace) against the branch that falls through to
//! it, firing when the two are textually equal.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct PreferIfNullOperators;

impl Rule for PreferIfNullOperators {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

use crate::lint::expressions::{equivalent, null_check};

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            span,
        } = node
            && let Some((operand, is_eq)) = null_check(condition)
        {
            // `x == null ? y : x` keeps `x` in the else branch;
            // `x != null ? x : y` keeps `x` in the then branch.
            let fallthrough = if is_eq { else_expr } else { then_expr };
            if equivalent(operand, fallthrough, self.ctx) {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-if-null-operators",
                    "Prefer using the if-null operator '??'.",
                    *span,
                ));
            }
        }
        walk_expr(self, node);
    }
}
