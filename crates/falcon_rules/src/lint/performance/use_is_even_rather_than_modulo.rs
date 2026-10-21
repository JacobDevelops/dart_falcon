//! Prefers `isEven`/`isOdd` to comparing `x % 2` with `0` or `1`.
//!
//! For an `int` `x`, `x % 2 == 0` is `x.isEven` and `x % 2 == 1` is `x.isOdd`;
//! the getters say what is meant. The rule fires on `==` whose left operand is
//! `<expr> % 2` with static type `int` and whose right operand is an integer
//! literal: `0` suggests `isEven`, any other literal `isOdd` (the official rule
//! does not check the value). Comparisons in a constant context (parameter
//! default values are not one), and in the assert initializers of a `const`
//! constructor, are exempt because `isEven` is not a constant expression.
//!
//! Adopted from the official `use_is_even_rather_than_modulo` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, parse_int};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::const_eval::ConstContexts;
use crate::lint::const_eval::ConstructorConstContexts as ConstConstructorAsserts;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseIsEvenRatherThanModulo;

impl Rule for UseIsEvenRatherThanModulo {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            contexts: ConstContexts::of(program),
            asserts: ConstConstructorAsserts::new(program),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    contexts: ConstContexts,
    asserts: ConstConstructorAsserts,
}

fn int_literal(expr: Expr<'_>) -> Option<i128> {
    match expr {
        Expr::IntLit(literal) => parse_int(literal.value()),
        _ => None,
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if binary.op() != BinaryOp::EqEq {
            return;
        }
        let left = binary.left();
        let right = binary.right();
        let span = binary.span();
        let Expr::Binary(modulo) = left else {
            return;
        };
        if modulo.op() != BinaryOp::Mod {
            return;
        }
        let divisor = modulo.right();
        let getter = match int_literal(right) {
            Some(0) => "isEven",
            Some(_) => "isOdd",
            None => return,
        };
        if int_literal(divisor) != Some(2)
            || !state.infer(left).interface("dart:core", "int")
            || self.contexts.contains(&span)
            || self.asserts.in_const_constructor_assert(&span)
        {
            return;
        }
        self.diags.push(self.ctx.diagnostic(format!("Use '{getter}' rather than '% 2'."), span));
    }
}
