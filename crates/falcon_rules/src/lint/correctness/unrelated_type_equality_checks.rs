//! Reports `==`/`!=` comparisons and relational `==`/`!=` patterns whose
//! operand types are unrelated.
//!
//! Adopted from the official `unrelated_type_equality_checks` lint. Relatedness
//! is the linter's `typesAreUnrelated` ([`TypeRelation::RuntimeUnrelatedTo`]),
//! with one exemption: a `package:fixnum` `Int32`/`Int64` on the left of an `int`
//! is comparable. Comparisons with a `null` literal operand are skipped.
//!
//! [`TypeRelation::RuntimeUnrelatedTo`]: falcon_analyze::TypeRelation::RuntimeUnrelatedTo

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth, TypeRelation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UnrelatedTypeEqualityChecks;

impl Rule for UnrelatedTypeEqualityChecks {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics) }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check(
        &mut self,
        left: &ResolvedType,
        right: &ResolvedType,
        span: Span,
        state: &SemanticState<'_>,
    ) {
        let fixnum_int = is_fixnum_int(left, state) && right.interface("dart:core", "int");
        if !fixnum_int
            && state
                .signatures
                .relation(left, TypeRelation::RuntimeUnrelatedTo(right), &state.model)
                == Truth::Yes
        {
            self.diagnostics.push(self.ctx.diagnostic("Equality checks should compare related types.", span));
        }
    }
}

/// The linter's `isFixnumIntX`: `Int32` or `Int64` from `package:fixnum`.
fn is_fixnum_int(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
    let ResolvedType::Interface { identity, .. } = ty else {
        return false;
    };
    matches!(identity.name(), "Int32" | "Int64")
        && (identity.is_package("fixnum", identity.name())
            || state
                .model
                .declaration_uri(identity)
                .is_some_and(|uri| uri.starts_with("package:fixnum/")))
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        if let Expr::Binary(binary) = node
            && matches!(binary.op(), BinaryOp::EqEq | BinaryOp::NotEq)
            && !matches!(binary.left().unparenthesized(), Expr::NullLit(_))
            && !matches!(binary.right().unparenthesized(), Expr::NullLit(_))
        {
            self.check(
                &state.infer(binary.left()),
                &state.infer(binary.right()),
                binary.op_span(),
                state,
            );
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if let Pattern::Relational(relational) = node
            && matches!(
                relational.op(),
                RelationalPatternOp::Eq | RelationalPatternOp::NotEq
            )
            && let Some(matched) = matched
        {
            self.check(
                matched,
                &state.infer(relational.value()),
                relational.span(),
                state,
            );
        }
    }
}
