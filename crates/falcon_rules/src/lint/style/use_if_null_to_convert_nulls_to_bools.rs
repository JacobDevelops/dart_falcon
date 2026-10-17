//! Prefers `??` to comparing a nullable `bool` against a boolean literal.
//!
//! For a `bool?` value `b`, `b == true` is `b ?? false` and `b != false` is
//! `b ?? true`; the if-null forms state the null default directly. The rule
//! fires on `==` with `true` and `!=` with `false` on the right when the left
//! operand's static type is proven to be `bool?`.
//!
//! Operand types include promotions proven at the comparison's use site.
//! Parenthesized boolean literals do not match the official rule.
//!
//! Adopted from the official `use_if_null_to_convert_nulls_to_bools` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions::is_parenthesized;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseIfNullToConvertNullsToBools;

impl Rule for UseIfNullToConvertNullsToBools {
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

fn is_nullable_bool(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Interface { identity, nullable: true, .. }
        if identity.is_sdk("dart:core", "bool"))
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Binary(binary) = node else {
            return;
        };
        let matches = match (binary.op(), binary.right()) {
            (BinaryOp::EqEq, Expr::BoolLit(literal)) => literal.value(),
            (BinaryOp::NotEq, Expr::BoolLit(literal)) => !literal.value(),
            _ => false,
        };
        if matches
            && !is_parenthesized(binary.right())
            && is_nullable_bool(&state.infer(binary.left()))
        {
            self.diags.push(self.ctx.diagnostic(
                "use-if-null-to-convert-nulls-to-bools",
                "Use an if-null operator to convert a 'null' to a 'bool'.",
                binary.span(),
            ));
        }
    }
}
