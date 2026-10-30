//! Reports redundant predicates in directly paired logical conditions.
//!
//! Integer comparisons against literals are checked for implication, including
//! reversed operands and inclusive bounds. Null checks paired with non-nullable
//! type tests are also checked. Only local variables and parameters are compared;
//! getters, calls, dynamic operators and floating-point comparisons are excluded.
//! This rule measures redundancy, not expression-chain length, and has no options.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::{interval, local_name, nested_integer_literal, reverse};

#[derive(Default)]
pub struct AvoidExcessiveExpressions;

impl Rule for AvoidExcessiveExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if !matches!(binary.op(), BinaryOp::And | BinaryOp::Or) {
            return;
        }
        let left = binary.left().unparenthesized();
        let right = binary.right().unparenthesized();
        let redundant = null_redundancy(left, right, binary.op(), state)
            .or_else(|| null_redundancy(right, left, binary.op(), state))
            .or_else(|| integer_redundancy(left, right, binary.op(), state));
        if let Some(expression) = redundant {
            self.diagnostics.push(self.ctx.diagnostic(
                "This condition is redundant. Remove it or replace it with a different condition.",
                expression.span(),
            ));
        }
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

fn null_redundancy<'a>(
    check: Expr<'a>,
    test: Expr<'a>,
    logical: BinaryOp,
    state: &SemanticState<'a>,
) -> Option<Expr<'a>> {
    let (Expr::Binary(check_binary), Expr::Is(test)) = (check, test) else {
        return None;
    };
    let expected = if logical == BinaryOp::And {
        BinaryOp::NotEq
    } else {
        BinaryOp::EqEq
    };
    if check_binary.op() != expected || test.negated() != (logical == BinaryOp::Or) {
        return None;
    }
    let ((value, Expr::NullLit(_)) | (Expr::NullLit(_), value)) = (
        check_binary.left().unparenthesized(),
        check_binary.right().unparenthesized(),
    ) else {
        return None;
    };
    if local_name(value, state)? != local_name(test.expr(), state)? {
        return None;
    }
    let ty = state
        .model
        .resolve_type_in(test.dart_type(), &state.type_parameters);
    state.signatures.is_non_nullable(&ty).then_some(check)
}

struct Comparison<'a> {
    name: &'a str,
    minimum: i128,
    maximum: i128,
    excluded: Option<i128>,
}

fn comparison<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> Option<Comparison<'a>> {
    let Expr::Binary(binary) = expression else {
        return None;
    };
    let left = binary.left().unparenthesized();
    let right = binary.right().unparenthesized();
    let (value, bound, op) = if let Some(bound) = nested_integer_literal(right) {
        (left, bound, binary.op())
    } else {
        (right, nested_integer_literal(left)?, reverse(binary.op())?)
    };
    let name = local_name(value, state)?;
    let ty = state.infer(value);
    if ty.nullable() || !ty.interface("dart:core", "int") {
        return None;
    }
    let (minimum, maximum, excluded) = interval(op, bound)?;
    Some(Comparison {
        name,
        minimum,
        maximum,
        excluded,
    })
}

fn implies(left: &Comparison<'_>, right: &Comparison<'_>) -> bool {
    left.minimum >= right.minimum
        && left.maximum <= right.maximum
        && right.excluded.is_none_or(|excluded| {
            excluded < left.minimum || excluded > left.maximum || left.excluded == Some(excluded)
        })
}

fn integer_redundancy<'a>(
    left: Expr<'a>,
    right: Expr<'a>,
    logical: BinaryOp,
    state: &SemanticState<'a>,
) -> Option<Expr<'a>> {
    let a = comparison(left, state)?;
    let b = comparison(right, state)?;
    if a.name != b.name {
        return None;
    }
    if implies(&a, &b) {
        Some(if logical == BinaryOp::And {
            right
        } else {
            left
        })
    } else if implies(&b, &a) {
        Some(if logical == BinaryOp::And {
            left
        } else {
            right
        })
    } else {
        None
    }
}
