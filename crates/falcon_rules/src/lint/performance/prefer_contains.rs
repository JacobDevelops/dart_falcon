//! Prefers `contains` to comparing the result of `indexOf` with a constant.
//!
//! Adopted from the official `prefer_contains` lint. The rule looks at a
//! comparison (`==`, `!=`, `<`, `<=`, `>`, `>=`) of an `Iterable` or `String`
//! receiver's `indexOf(...)` call (optionally behind `as` casts) with an
//! integer constant `<= 0` on the other side: an integer literal, a negated
//! literal or identifier, or an identifier naming an `int` constant. When the
//! call has exactly two arguments the second (the start index) must be the
//! constant `0`. Comparisons with `-1` (`==`, `!=`, `<=` and `>` after
//! normalizing the call to the left) and `0` (`>=`, `<`) are membership tests
//! that `contains` states directly; comparisons that are always true or false
//! (`< -1`, `>= -1`, or any comparison with a value below `-1`) are reported
//! too. A constant on the right is preferred: when the right operand is an
//! integer constant the left operand is not considered.

use falcon_analyze::constant_value::Number;
use falcon_analyze::{
    AnalyzeContext, ConstantValue, ResolvedType, Rule, RulePreparation, Truth,
    evaluate_constant_with_local_values, parse_int,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

const RULE: &str = "prefer-contains";

#[derive(Default)]
pub struct PreferContains;

impl Rule for PreferContains {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// What a comparison of `indexOf` with a constant amounts to.
enum Finding {
    UseContains,
    AlwaysFalse,
    AlwaysTrue,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op,
            left,
            right,
            span,
            ..
        } = node
        else {
            return;
        };
        if !matches!(
            op,
            BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
        ) {
            return;
        }
        let finding = match int_value(right, state) {
            Some(value) => (value <= 0 && unassigned_index_of(left, state))
                .then(|| finding(value, *op))
                .flatten(),
            None => int_value(left, state).and_then(|value| {
                (value <= 0 && unassigned_index_of(right, state))
                    .then(|| finding(value, inverted(*op)))
                    .flatten()
            }),
        };
        let message = match finding {
            Some(Finding::UseContains) => {
                "Use 'contains' instead of comparing the result of 'indexOf'."
            }
            Some(Finding::AlwaysFalse) => "This comparison of 'indexOf' is always false.",
            Some(Finding::AlwaysTrue) => "This comparison of 'indexOf' is always true.",
            None => return,
        };
        self.diags.push(self.ctx.diagnostic(RULE, message, *span));
    }
}

/// The comparison `indexOf(...) <op> value` for a `value <= 0`.
fn finding(value: i128, op: BinaryOp) -> Option<Finding> {
    use BinaryOp::*;
    match (value, op) {
        (-1, EqEq | NotEq | LtEq | Gt) | (0, GtEq | Lt) => Some(Finding::UseContains),
        (-1, Lt) => Some(Finding::AlwaysFalse),
        (-1, GtEq) => Some(Finding::AlwaysTrue),
        (value, EqEq | LtEq | Lt) if value < -1 => Some(Finding::AlwaysFalse),
        (value, NotEq | GtEq | Gt) if value < -1 => Some(Finding::AlwaysTrue),
        _ => None,
    }
}

/// The operator with its operands swapped.
fn inverted(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::LtEq => BinaryOp::GtEq,
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::GtEq => BinaryOp::LtEq,
        op => op,
    }
}

/// Whether `expression`, behind `as` casts, invokes `indexOf` on an explicit
/// `Iterable` or `String` receiver, with a zero start index when it has two
/// arguments.
fn unassigned_index_of(mut expression: &Expr, state: &SemanticState<'_>) -> bool {
    while let Expr::As { expr, .. } = expression.unparenthesized() {
        expression = expr;
    }
    let Expr::Call { callee, args, .. } = expression.unparenthesized() else {
        return false;
    };
    let Expr::Field { object, field, .. } = callee.unparenthesized() else {
        return false;
    };
    if field.name != "indexOf" {
        return false;
    }
    let receiver = state.infer(object);
    if state
        .signatures
        .interface_check_sdk(&receiver, "dart:core", "Iterable", &state.model)
        != Truth::Yes
        && state
            .signatures
            .interface_check_sdk(&receiver, "dart:core", "String", &state.model)
            != Truth::Yes
    {
        return false;
    }
    let mut arguments = args
        .positional
        .iter()
        .chain(args.named.iter().map(|argument| &argument.value));
    match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(_), Some(start), None) => int_value(start, state) == Some(0),
        _ => true,
    }
}

/// The linter's `getIntValue`: an integer literal or an identifier naming an
/// integer constant, optionally negated.
fn int_value(expression: &Expr, state: &SemanticState<'_>) -> Option<i128> {
    match expression.unparenthesized() {
        Expr::Unary {
            op: UnaryOp::Minus,
            operand,
            ..
        } => unsigned_int_value(operand, state)?.checked_neg(),
        expression => unsigned_int_value(expression, state),
    }
}

fn unsigned_int_value(expression: &Expr, state: &SemanticState<'_>) -> Option<i128> {
    match expression.unparenthesized() {
        Expr::IntLit { value, .. } => parse_int(value),
        Expr::Ident(identifier) => {
            let constants = state.visible_local_constants();
            if (state.environment.is_bound(&identifier.name)
                && !constants.contains_key(identifier.name.as_str()))
                || !is_int_constant(expression, &identifier.name, state)
            {
                return None;
            }
            match evaluate_constant_with_local_values(
                expression,
                None,
                &constants,
                &state.model,
                &state.signatures,
            )? {
                ConstantValue::Number(Number::Int(value)) => Some(value),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether the identifier `expression` names a constant of static type `int`
/// (the evaluator folds integral doubles into integers, so `-1.0` must be told
/// apart here).
fn is_int_constant(expression: &Expr, name: &str, state: &SemanticState<'_>) -> bool {
    let declared = state.infer(expression);
    if declared.interface("dart:core", "int") {
        return true;
    }
    matches!(declared, ResolvedType::Unknown)
        && state
            .model
            .resolve_value(name)
            .and_then(|identity| state.signatures.constant_initializer(&identity))
            .is_some_and(|initializer| state.infer(initializer).interface("dart:core", "int"))
}
