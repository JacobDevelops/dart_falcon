//! Flags boolean literals compared or combined with a non-nullable `bool`.
//!
//! `x == true` is just `x`, `x != true` is `!x`, `x && true` is `x`, and so on
//! for `||`, `&`, `|`, and `^`. The literal is only redundant when the other
//! operand is provably a non-nullable `bool`; a `bool?` compared to `true` is a
//! meaningful null check and is left alone.
//! Parenthesized literals are separate expressions and are not reported.
//!
//! ```dart
//! if (done == true) {} // flagged
//! if (done) {}         // ok
//! ```
//!
//! Adopted from the official `no_literal_bool_comparisons` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::Program;
use falcon_syntax::cst::{BinaryOp, Expr};

use crate::lint::expressions::is_parenthesized;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct NoLiteralBoolComparisons;

impl Rule for NoLiteralBoolComparisons {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

fn non_nullable_bool(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let ty = state.infer(expression);
    ty.interface("dart:core", "bool") && !ty.nullable()
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if !matches!(
            binary.op(),
            BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::BitOr
                | BinaryOp::Or
                | BinaryOp::BitAnd
                | BinaryOp::And
                | BinaryOp::BitXor
        ) {
            return;
        }
        let (left, right) = (binary.left(), binary.right());
        // A parenthesized literal is not a BooleanLiteral operand.
        let is_literal =
            |operand: Expr<'_>| matches!(operand, Expr::BoolLit(_)) && !is_parenthesized(operand);
        let literal = if is_literal(right) && non_nullable_bool(left, state) {
            right
        } else if is_literal(left) && non_nullable_bool(right, state) {
            left
        } else {
            return;
        };
        self.diags.push(self.ctx.diagnostic(
            "no-literal-bool-comparisons",
            "Unnecessary comparison to a boolean literal.",
            literal.span(),
        ));
    }
}
