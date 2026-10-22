//! Prefer context.read/watch when provider extensions are in scope.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::value_name_is_shadowed;

#[derive(Default)]
pub struct PreferProviderExtensions;

impl Rule for PreferProviderExtensions {
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

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expression else {
            return;
        };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return;
        };
        if field.field().name() != "of" || field.is_null_safe() {
            return;
        }
        let Some(segments) = falcon_analyze::expression_segments(field.object(), false) else {
            return;
        };
        if segments
            .first()
            .is_some_and(|head| value_name_is_shadowed(head, state))
        {
            return;
        }
        let Some(identity) = state.model.resolve_dotted(&segments) else {
            return;
        };
        if identity.name() != "Provider"
            || !(identity.is_package("provider", "Provider")
                || state
                    .model
                    .declaration_uri(&identity)
                    .is_some_and(|uri| uri.starts_with("package:provider/")))
        {
            return;
        }
        let mut positional = call.args().positional();
        let (Some(context), None) = (positional.next(), positional.next()) else {
            return;
        };
        let receiver = state.infer(context);
        if !matches!(&receiver, ResolvedType::Interface { identity, .. } if identity.is_package("flutter", "BuildContext"))
        {
            return;
        }
        let mut method = "watch";
        for argument in call.args().named() {
            if argument.name().name() != "listen" {
                return;
            }
            let Expr::BoolLit(value) = argument.value().unparenthesized() else {
                return;
            };
            method = if value.value() { "watch" } else { "read" };
        }
        let Some((Some(owner), _, _)) =
            state
                .signatures
                .extension_method(&receiver, method, &state.model)
        else {
            return;
        };
        if !(owner.is_package("provider", owner.name())
            || state
                .model
                .declaration_uri(owner)
                .is_some_and(|uri| uri.starts_with("package:provider/")))
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(if method == "read" {
            "Use `context.read` instead of `Provider.of`."
        } else {
            "Use `context.watch` instead of `Provider.of`."
        }, call.callee().span()));
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
