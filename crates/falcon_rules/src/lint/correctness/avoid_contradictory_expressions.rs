//! Flags pairs of conditions that are always false with `&&` or true with `||`.
//!
//! Checks a stable boolean against its negation, complementary primitive equality
//! comparisons, and incompatible integer comparisons against integer literals.
//! Only directly paired conditions (allowing parentheses) are compared. Reads must
//! be local variables or parameters; getters, fields, calls, dynamic values, and
//! user-defined operators are excluded. Integer bounds use exact arithmetic;
//! floating-point bounds are excluded because NaN invalidates total ordering.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::{equivalent, integer_literal, interval, reverse, stable_local};

#[derive(Default)]
pub struct AvoidContradictoryExpressions;

impl Rule for AvoidContradictoryExpressions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if !matches!(binary.op(), BinaryOp::And | BinaryOp::Or) {
            return;
        }
        let left = binary.left().unparenthesized();
        let right = binary.right().unparenthesized();
        if !opposite_booleans(left, right, state, self.ctx)
            && !opposite_equalities(left, right, state, self.ctx)
            && !incompatible_integers(left, right, binary.op(), state, self.ctx)
        {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            if binary.op() == BinaryOp::And {
                "These conditions are always false together. Correct the contradictory comparison."
            } else {
                "These conditions are always true together. Correct the contradictory comparison."
            },
            binary.span(),
        ));
    }
}

fn stable_primitive(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let expression = expression.unparenthesized();
    let stable = match expression {
        Expr::Ident(identifier) => stable_local(identifier.name(), state),
        Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) => true,
        Expr::StringLit(string) => string.interpolations().next().is_none(),
        _ => false,
    };
    if !stable {
        return false;
    }
    let ty = state.infer(expression);
    matches!(expression, Expr::NullLit(_))
        || (!ty.nullable()
            && ["bool", "int", "double", "num", "String"]
                .iter()
                .any(|name| ty.interface("dart:core", name)))
}

fn opposite_booleans(
    left: Expr<'_>,
    right: Expr<'_>,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    let (plain, negated) = match (left, right) {
        (plain, Expr::Unary(unary)) | (Expr::Unary(unary), plain)
            if unary.op() == UnaryOp::Bang =>
        {
            (plain, unary.operand().unparenthesized())
        }
        _ => return false,
    };
    stable_primitive(plain, state)
        && state.infer(plain).interface("dart:core", "bool")
        && equivalent(plain, negated, ctx)
}

fn opposite_equalities(
    left: Expr<'_>,
    right: Expr<'_>,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    let (Expr::Binary(left), Expr::Binary(right)) = (left, right) else {
        return false;
    };
    if !matches!(
        (left.op(), right.op()),
        (BinaryOp::EqEq, BinaryOp::NotEq) | (BinaryOp::NotEq, BinaryOp::EqEq)
    ) {
        return false;
    }
    let (a, b) = (
        left.left().unparenthesized(),
        left.right().unparenthesized(),
    );
    let (c, d) = (
        right.left().unparenthesized(),
        right.right().unparenthesized(),
    );
    [a, b, c, d]
        .into_iter()
        .all(|expression| stable_primitive(expression, state))
        && ((equivalent(a, c, ctx) && equivalent(b, d, ctx))
            || (equivalent(a, d, ctx) && equivalent(b, c, ctx)))
}

struct IntegerComparison<'a> {
    value: Expr<'a>,
    op: BinaryOp,
    bound: i128,
}

fn integer_comparison<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
) -> Option<IntegerComparison<'a>> {
    let Expr::Binary(binary) = expression else {
        return None;
    };
    let left = binary.left().unparenthesized();
    let right = binary.right().unparenthesized();
    let (value, op, bound) = if let Some(bound) = integer_literal(right) {
        (left, binary.op(), bound)
    } else {
        (right, reverse(binary.op())?, integer_literal(left)?)
    };
    let Expr::Ident(identifier) = value else {
        return None;
    };
    if !stable_local(identifier.name(), state)
        || !state.infer(value).interface("dart:core", "int")
        || state.infer(value).nullable()
    {
        return None;
    }
    interval(op, bound)?;
    Some(IntegerComparison { value, op, bound })
}

fn negate(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::EqEq => BinaryOp::NotEq,
        BinaryOp::NotEq => BinaryOp::EqEq,
        BinaryOp::Lt => BinaryOp::GtEq,
        BinaryOp::LtEq => BinaryOp::Gt,
        BinaryOp::Gt => BinaryOp::LtEq,
        BinaryOp::GtEq => BinaryOp::Lt,
        _ => op,
    }
}

fn incompatible_integers<'a>(
    left: Expr<'a>,
    right: Expr<'a>,
    logical: BinaryOp,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    let (Some(left), Some(right)) = (
        integer_comparison(left, state),
        integer_comparison(right, state),
    ) else {
        return false;
    };
    if !equivalent(left.value, right.value, ctx) {
        return false;
    }
    let operation = |op| {
        if logical == BinaryOp::Or {
            negate(op)
        } else {
            op
        }
    };
    let Some((left_min, left_max, left_excluded)) = interval(operation(left.op), left.bound) else {
        return false;
    };
    let Some((right_min, right_max, right_excluded)) = interval(operation(right.op), right.bound)
    else {
        return false;
    };
    let minimum = left_min.max(right_min);
    let maximum = left_max.min(right_max);
    minimum > maximum
        || (minimum == maximum
            && (left_excluded == Some(minimum) || right_excluded == Some(minimum)))
}
