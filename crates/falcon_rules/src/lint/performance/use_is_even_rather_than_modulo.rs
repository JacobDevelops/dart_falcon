//! Prefers `isEven`/`isOdd` to comparing `x % 2` with `0` or `1`.
//!
//! For an `int` `x`, `x % 2 == 0` is `x.isEven` and `x % 2 == 1` is `x.isOdd`;
//! the getters say what is meant. The rule fires on `==` whose left operand is
//! `<expr> % 2` with static type `int` and whose right operand is the literal `0`
//! or `1`. Comparisons in a constant context, and in the assert initializers of a
//! `const` constructor, are exempt because `isEven` is not a constant expression.
//!
//! Adopted from the official `use_is_even_rather_than_modulo` lint.

use falcon_analyze::{AnalyzeContext, Rule, parse_int};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::style::use_named_constants::ConstContexts;

#[derive(Default)]
pub struct UseIsEvenRatherThanModulo;

impl Rule for UseIsEvenRatherThanModulo {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            contexts: ConstContexts::new(program),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    contexts: ConstContexts,
}

fn int_literal(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::IntLit { value, .. } => parse_int(value),
        _ => None,
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op: BinaryOp::EqEq,
            left,
            right,
            span,
        } = node
        else {
            return;
        };
        let getter = match int_literal(right) {
            Some(0) => "isEven",
            Some(1) => "isOdd",
            _ => return,
        };
        let Expr::Binary {
            op: BinaryOp::Mod,
            right: divisor,
            ..
        } = left.as_ref()
        else {
            return;
        };
        if int_literal(divisor) != Some(2)
            || !state.infer(left).interface("dart:core", "int")
            || self.contexts.contains(span)
            || self.contexts.in_const_constructor_assert(span)
        {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            "use-is-even-rather-than-modulo",
            format!("Use '{getter}' rather than '% 2'."),
            *span,
        ));
    }
}
