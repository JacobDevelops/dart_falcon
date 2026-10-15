//! Reports `==`/`!=` comparisons and relational `==`/`!=` patterns whose
//! operand types are unrelated.
//!
//! Adopted from the official `unrelated_type_equality_checks` lint. Relatedness
//! is the linter's `typesAreUnrelated` ([`SignatureIndex::types_are_unrelated`]),
//! with one exemption: a `package:fixnum` `Int32`/`Int64` on the left of an `int`
//! is comparable. Comparisons with a `null` literal operand are skipped.
//!
//! [`SignatureIndex::types_are_unrelated`]: falcon_analyze::SignatureIndex::types_are_unrelated

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnrelatedTypeEqualityChecks;

impl Rule for UnrelatedTypeEqualityChecks {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
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
                .types_are_unrelated(left, right, &state.model)
                == Truth::Yes
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "unrelated-type-equality-checks",
                "Equality checks should compare related types.",
                span,
            ));
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

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Binary {
            op: BinaryOp::EqEq | BinaryOp::NotEq,
            op_span,
            left,
            right,
            ..
        } = node
            && !matches!(left.unparenthesized(), Expr::NullLit { .. })
            && !matches!(right.unparenthesized(), Expr::NullLit { .. })
        {
            self.check(&state.infer(left), &state.infer(right), *op_span, state);
        }
    }

    fn visit_pattern(
        &mut self,
        node: &Pattern,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if let Pattern::Relational {
            op: RelationalPatternOp::Eq | RelationalPatternOp::NotEq,
            value,
            span,
        } = node
            && let Some(matched) = matched
        {
            self.check(matched, &state.infer(value), *span, state);
        }
    }
}
