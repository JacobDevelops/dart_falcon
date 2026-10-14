//! Flags boolean literals compared or combined with a non-nullable `bool`.
//!
//! `x == true` is just `x`, `x != true` is `!x`, `x && true` is `x`, and so on
//! for `||`, `&`, `|`, and `^`. The literal is only redundant when the other
//! operand is provably a non-nullable `bool`; a `bool?` compared to `true` is a
//! meaningful null check and is left alone.
//!
//! ```dart
//! if (done == true) {} // flagged
//! if (done) {}         // ok
//! ```
//!
//! Adopted from the official `no_literal_bool_comparisons` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::{BinaryOp, Expr};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct NoLiteralBoolComparisons;

impl Rule for NoLiteralBoolComparisons {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

fn non_nullable_bool(expression: &Expr, state: &SemanticState<'_>) -> bool {
    let ty = state.infer(expression);
    ty.interface("dart:core", "bool") && !ty.nullable()
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Binary {
            op:
                BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::BitOr
                | BinaryOp::Or
                | BinaryOp::BitAnd
                | BinaryOp::And
                | BinaryOp::BitXor,
            left,
            right,
            ..
        } = node
        else {
            return;
        };
        let literal = match (left.as_ref(), right.as_ref()) {
            (operand, literal @ Expr::BoolLit { .. }) if non_nullable_bool(operand, state) => {
                literal
            }
            (literal @ Expr::BoolLit { .. }, operand) if non_nullable_bool(operand, state) => {
                literal
            }
            _ => return,
        };
        self.diags.push(self.ctx.diagnostic(
            "no-literal-bool-comparisons",
            "Unnecessary comparison to a boolean literal.",
            *literal.span(),
        ));
    }
}
