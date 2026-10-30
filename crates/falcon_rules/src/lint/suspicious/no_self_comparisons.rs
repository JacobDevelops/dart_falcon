//! Flags proven self-comparisons of side-effect-free scalar values.
//!
//! Getter reads, index operations, calls, and user-defined operator receivers are
//! intentionally excluded: evaluating the same syntax twice need not produce the
//! same value, and equality/ordering may be overloaded.
//! A scalar read type does not establish stability: unqualified top-level and
//! inherited getter names still execute a getter on each evaluation.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct NoSelfComparisons;

impl Rule for NoSelfComparisons {
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
        let left = binary.left();
        let right = binary.right();
        if !is_comparison(binary.op())
            || !proven_scalar_value(left, state)
            || !proven_scalar_value(right, state)
        {
            return;
        }
        if !equivalent(left, right, self.ctx) {
            return;
        }
        self.diags.push(self.ctx.diagnostic("Both operands of this comparison are identical.", binary.span()));
    }
}

fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
    )
}

fn proven_scalar_value<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let mut proof = ScalarValueProof {
        state,
        proven: false,
    };
    proof.visit_expr(expression);
    proof.proven
}

/// Only stable scalar leaves and non-mutating unary operations qualify.
/// Shared traversal supplies the unary operand's proof.
struct ScalarValueProof<'s, 'a> {
    state: &'s SemanticState<'a>,
    proven: bool,
}

impl<'a> Visitor<'a> for ScalarValueProof<'_, 'a> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        let syntactically_stable = match node {
            Expr::Ident(identifier) => self.state.is_variable_read(identifier.name()),
            Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) => true,
            Expr::StringLit(string) => string.interpolations().next().is_none(),
            // `++x`/`--x` mutate, so the two sides are not the same value.
            Expr::Unary(unary)
                if !matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus)
                    && matches!(
                        unary.operand().unparenthesized(),
                        Expr::Ident(_) | Expr::IntLit(_) | Expr::DoubleLit(_)
                    ) =>
            {
                walk_expr(self, node);
                self.proven
            }
            _ => false,
        };
        self.proven = syntactically_stable && scalar_type(&self.state.infer(node));
    }
}

fn scalar_type(ty: &ResolvedType) -> bool {
    matches!(ty.view(), ResolvedTypeView::Null)
        || matches!(ty.view(), ResolvedTypeView::Interface {
            identity: DeclarationIdentity::Sdk { library, name }, ..
        } if library.as_str() == "dart:core"
            && matches!(name.as_str(), "bool" | "int" | "double" | "num" | "String"))
}

use crate::lint::expressions::equivalent;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::test_support::diagnostics;
    use std::path::Path;

    #[test]
    fn source_getters_are_not_stable_variable_reads() {
        let source = r"
int nextId = 0;
int get changing => nextId++;
bool getterEqual() => changing == changing;
bool getterUnary() => -changing == -changing;
bool globalEqual() => nextId == nextId;
bool parameterEqual(int changing) => changing == changing;
bool localEqual() { final changing = 1; return changing == changing; }
bool primitiveEqual() => -1 == -1;
class Values {
  int value = 0;
  int get changing => value++;
  bool getterEqual() => changing == changing;
  bool getterUnary() => -changing == -changing;
  bool fieldEqual() => value == value;
}
class Child extends Values {
  bool inheritedGetterEqual() => changing == changing;
  bool inheritedFieldEqual() => value == value;
}
";
        let diagnostics = diagnostics(
            NoSelfComparisons::NAME,
            &NoSelfComparisons,
            Path::new("self_comparisons.dart"),
            source,
            true,
        );
        let lines: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| source[..diagnostic.span.start].matches('\n').count() + 1)
            .collect();
        assert_eq!(lines, [6, 7, 8, 9, 15, 19], "{diagnostics:?}");
    }
}
