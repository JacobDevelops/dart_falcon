//! Reports filtered iterable searches that have a direct search-method alternative.
//!
//! On confirmed SDK `Iterable` APIs, `where(test).first`, `.last`, and `.single`
//! can use `firstWhere`, `lastWhere`, and `singleWhere` without creating a
//! filtering iterable. The `lastWhere` recommendation requires a pure predicate:
//! a list may search backward and stop early, changing predicate evaluation
//! order and count. No automatic fix is offered. Lazy `expand` calls are not
//! reported: eagerly spreading them could change evaluation timing.
//!
//! Only explicit, non-null-aware chains with one positional callback match.
//! Source overrides, unrelated lookalike APIs, and unresolved receivers are
//! excluded. Arrow closures testing their parameter with `is T` belong to
//! `prefer-iterable-where-type`; length and membership checks belong to
//! `prefer-is-empty` and `prefer-contains`, respectively. No arbitrary `sync*`
//! function is classified as slow merely because it is a generator.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidSlowCollectionMethods;

impl Rule for AvoidSlowCollectionMethods {
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

/// Keep this rule disjoint from the existing arrow-closure type-filter lint.
fn is_type_filter(predicate: Expr<'_>) -> bool {
    let Expr::FuncExpr(function) = predicate.unparenthesized() else {
        return false;
    };
    let mut parameters = function.params().params();
    let (Some(parameter), None) = (parameters.next(), parameters.next()) else {
        return false;
    };
    parameter.kind() == ParamKind::Positional
        && matches!(function.body(), FunctionBody::Arrow(body)
            if matches!(body.expr().unparenthesized(), Expr::Is(test)
                if !test.negated()
                    && matches!(test.expr().unparenthesized(), Expr::Ident(identifier)
                        if identifier.name() == parameter.name().name())))
}

fn sdk_iterable_member(field: FieldExpr<'_>, state: &SemanticState<'_>) -> bool {
    !field.is_null_safe()
        && state.signatures.resolved_member_owner(
            &state.infer(field.object()),
            field.field().name(),
            &state.model,
        ) == Some(DeclarationIdentity::sdk("dart:core", "Iterable"))
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Field(field) = node else {
            return;
        };
        if field.is_null_safe() {
            return;
        }
        let message = match field.field().name() {
            "first" => "Use `firstWhere` instead of `where(...).first`.",
            "last" => {
                "With a pure predicate, use `lastWhere`; predicate evaluation order and count may change."
            }
            "single" => "Use `singleWhere` instead of `where(...).single`.",
            _ => return,
        };
        let Expr::Call(call) = field.object().unparenthesized() else {
            return;
        };
        let Expr::Field(inner) = call.callee().unparenthesized() else {
            return;
        };
        if inner.field().name() != "where"
            || !call.args().named().is_empty()
            || !sdk_iterable_member(inner, state)
        {
            return;
        }
        let mut arguments = call.args().positional();
        let (Some(predicate), None) = (arguments.next(), arguments.next()) else {
            return;
        };
        if is_type_filter(predicate) {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(message, field.field().span()));
    }
}
