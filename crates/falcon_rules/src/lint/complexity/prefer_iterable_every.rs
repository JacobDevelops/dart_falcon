//! Flags `iterable.where(predicate).isEmpty` and filtered-length comparisons.
//!
//! An empty filtered iterable means every element fails the predicate. Prefer
//! `iterable.every((element) => !predicate(element))` to express that condition
//! directly. When a filtered length equals the original iterable's length, use
//! `iterable.every(predicate)` instead. Both forms can stop at the first failure.
//! Only resolved SDK `Iterable.where` calls are checked. Length comparisons
//! require the same simple receiver or explicit `this` field on both sides.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, RulePreparation};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferIterableEvery;

impl Rule for PreferIterableEvery {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

fn is_where_is_empty<'a>(expr: Expr<'a>, state: &SemanticState<'a>) -> Option<Span> {
    if let Expr::Field(field) = expr.unparenthesized()
        && field.field().name() == "isEmpty"
        && let Expr::Call(call) = field.object().unparenthesized()
        && let Expr::Field(where_field) = call.callee().unparenthesized()
        && where_field.field().name() == "where"
        && !field.is_null_safe()
        && is_iterable_where(call, where_field, state)
    {
        let mut parent = expr.syntax().parent();
        let mut negated = false;
        while let Some(node) = parent.and_then(Expr::cast) {
            match node {
                Expr::Paren(_) => {}
                Expr::Unary(unary) if unary.op() == UnaryOp::Bang => negated = !negated,
                Expr::Binary(binary) if matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) => {
                    let ((Expr::BoolLit(literal), _) | (_, Expr::BoolLit(literal))) = (
                        binary.left().unparenthesized(),
                        binary.right().unparenthesized(),
                    ) else {
                        break;
                    };
                    negated ^= literal.value() == (binary.op() == BinaryOp::NotEq);
                }
                _ => break,
            }
            parent = parent.and_then(SyntaxNode::parent);
        }
        return (!negated).then_some(field.span());
    }
    None
}

fn is_where_length_eq_length<'a>(expr: Expr<'a>, state: &SemanticState<'a>) -> Option<Span> {
    if let Expr::Binary(binary) = expr.unparenthesized()
        && binary.op() == BinaryOp::EqEq
        && let Expr::Field(left) = binary.left().unparenthesized()
        && left.field().name() == "length"
        && let Expr::Call(call) = left.object().unparenthesized()
        && let Expr::Field(where_field) = call.callee().unparenthesized()
        && where_field.field().name() == "where"
        && let Expr::Field(right) = binary.right().unparenthesized()
        && right.field().name() == "length"
        && !left.is_null_safe()
        && !right.is_null_safe()
        && is_iterable_where(call, where_field, state)
        && same_receiver(where_field.object(), right.object())
    {
        return Some(binary.span());
    }
    None
}

fn is_iterable_where<'a>(
    call: CallExpr<'a>,
    field: FieldExpr<'a>,
    state: &SemanticState<'a>,
) -> bool {
    !field.is_null_safe()
        && call.args().positional().count() == 1
        && call.args().named().is_empty()
        && state.signatures.resolved_member_owner(
            &state.infer(field.object()),
            "where",
            &state.model,
        ) == Some(DeclarationIdentity::sdk("dart:core", "Iterable"))
}

fn same_receiver(left: Expr<'_>, right: Expr<'_>) -> bool {
    match (left.unparenthesized(), right.unparenthesized()) {
        (Expr::Ident(left), Expr::Ident(right)) => left.name() == right.name(),
        (Expr::Field(left), Expr::Field(right)) => {
            !left.is_null_safe()
                && !right.is_null_safe()
                && matches!(left.object().unparenthesized(), Expr::This(_))
                && matches!(right.object().unparenthesized(), Expr::This(_))
                && left.field().name() == right.field().name()
        }
        _ => false,
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(node, Expr::Paren(_)) {
            return;
        }
        let matched = is_where_is_empty(node, state)
            .map(|span| (span, "Use `every((element) => !predicate(element))` instead of `where(predicate).isEmpty`."))
            .or_else(|| is_where_length_eq_length(node, state)
                .map(|span| (span, "Use `every(predicate)` instead of comparing the filtered and original iterable lengths.")));
        if let Some((span, message)) = matched {
            self.diags.push(self.ctx.diagnostic(message, span));
        }
    }
}
