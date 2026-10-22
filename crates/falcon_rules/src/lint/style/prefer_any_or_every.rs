//! Prefer `Iterable.every` to `where(predicate).isEmpty`.
//!
//! The complementary `.isNotEmpty` pattern belongs to `prefer-iterable-any`.
//! Only SDK Iterable.where calls with one positional predicate are matched, not
//! unresolved lookalikes, overrides, or null-aware chains. An every callback must
//! negate the original predicate; no automatic fix is offered.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferAnyOrEvery;

impl Rule for PreferAnyOrEvery {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Field(empty) = expression else {
            return;
        };
        if empty.is_null_safe() || empty.field().name() != "isEmpty" {
            return;
        }
        let mut parent = expression.syntax().parent();
        while parent
            .and_then(Expr::cast)
            .is_some_and(|node| matches!(node, Expr::Paren(_)))
        {
            parent = parent.and_then(|node| node.parent());
        }
        if matches!(parent.and_then(Expr::cast), Some(Expr::Unary(unary)) if unary.op() == UnaryOp::Bang)
        {
            // Negated filtered emptiness checks whether any element matches.
            return;
        }
        let Expr::Call(call) = empty.object().unparenthesized() else {
            return;
        };
        let Expr::Field(filter) = call.callee().unparenthesized() else {
            return;
        };
        if filter.is_null_safe()
            || filter.field().name() != "where"
            || call.args().positional().count() != 1
            || !call.args().named().is_empty()
            || state.signatures.resolved_member_owner(
                &state.infer(filter.object()),
                "where",
                &state.model,
            ) != Some(DeclarationIdentity::sdk("dart:core", "Iterable"))
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("This filtered iterable is only checked for emptiness. Use `.every((element) => !predicate(element))` instead of `.where(predicate).isEmpty`.", empty.span()));
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
