//! Prefer `Text.rich` over Flutter's `RichText` for accessible text scaling and
//! inherited default text styles. Constructor identity is resolved through import
//! prefixes and type aliases; dot-shorthand invocations use their expected type.
//! Unrelated declarations, shadowed names, and constructor tear-offs are ignored.

use falcon_analyze::{
    AnalyzeContext, ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticRuleVisitor,
    SemanticState,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{Expr, Program};

/// Prefer Flutter's `Text.rich` constructor over `RichText` for accessible text
/// scaling and inherited default text styles. Only resolved Flutter constructors
/// are reported; unrelated declarations and shadowed names are left unchanged.
#[derive(Default)]
pub struct PreferTextRich;

impl Rule for PreferTextRich {
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
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some(index) = self.ctx.identities() else {
            return;
        };
        let is_rich_text = if let Expr::Call(call) = node
            && matches!(call.callee().unparenthesized(), Expr::DotShorthand(_))
        {
            matches!(
                state.expected().map(ResolvedType::view),
                Some(ResolvedTypeView::Interface { identity, .. })
                    if index.is_package_declaration(identity, "flutter", "RichText")
            )
        } else {
            crate::lint::flutter::scoped_constructor_invocation(node, state).is_some_and(
                |(identity, _, _)| index.is_package_declaration(&identity, "flutter", "RichText"),
            )
        };
        if is_rich_text {
            self.diagnostics.push(self.ctx.diagnostic(
                "Use `Text.rich` instead of `RichText`.",
                crate::lint::flutter::constructor_span(node),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
