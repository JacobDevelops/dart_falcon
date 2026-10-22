//! Flags equality comparisons between non-constant SDK collections.
//!
//! List, Set, and Map equality compares identity rather than contents. Use a
//! collection equality helper instead. Comparisons between two constants are
//! exempt, as are unrelated types (covered by unrelated-type-equality-checks),
//! null comparisons, and custom collection classes that may override equality.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth, TypeRelation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::const_eval::constant_in_context;

#[derive(Default)]
pub struct AvoidCollectionEqualityChecks;

impl Rule for AvoidCollectionEqualityChecks {
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

fn collection(ty: &ResolvedType) -> bool {
    ["List", "Set", "Map"]
        .iter()
        .any(|name| ty.interface("dart:core", name))
}

fn constant(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::List(list) => list.is_const(),
        Expr::Set(set) => set.is_const(),
        Expr::Map(map) => map.is_const(),
        reference @ (Expr::Ident(_) | Expr::Field(_)) => constant_in_context(reference, &[], state),
        _ => false,
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        if !matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq) {
            return;
        }
        let left = state.infer(binary.left());
        let right = state.infer(binary.right());
        if collection(&left)
            && collection(&right)
            && state
                .signatures
                .relation(&left, TypeRelation::RuntimeUnrelatedTo(&right), &state.model)
                != Truth::Yes
            && !(constant(binary.left(), state) && constant(binary.right(), state))
        {
            self.diagnostics.push(self.ctx.diagnostic("Collection equality compares identity — use a content equality helper instead.", binary.op_span()));
        }
    }
}
