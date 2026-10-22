//! Flatten directly nested package:flutter_bloc BlocProviders with MultiBlocProvider.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
#[derive(Default)]
pub struct PreferMultiBlocProvider;
impl Rule for PreferMultiBlocProvider {
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
        let Some(args) = provider_args(expression, state, self.ctx) else {
            return;
        };
        let Some(child) = super::widget_replacements::argument(args, "child") else {
            return;
        };
        if provider_args(child, state, self.ctx).is_some() {
            self.diagnostics.push(self.ctx.diagnostic("Combine nested `BlocProvider` widgets with `MultiBlocProvider`.", crate::lint::flutter::constructor_span(expression)));
        }
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

// Generic named calls (BlocProvider<T>.value) wrap the receiver in a generic
// instantiation, which the general Flutter lint constructor helper rejects.
fn provider_args<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'_>,
) -> Option<ArgList<'a>> {
    let (identity, constructor, args) = match expression.unparenthesized() {
        Expr::Call(call) => {
            let segments = falcon_analyze::expression_segments(call.callee(), true)?;
            if crate::lint::flutter::value_name_is_shadowed(segments.first()?, state) {
                return None;
            }
            let (identity, constructor) =
                falcon_analyze::resolve_call_constructor(call.callee(), &state.model, true)?;
            (state.model.unaliased(identity), constructor, call.args())
        }
        _ => crate::lint::flutter::scoped_constructor_invocation(expression, state)?,
    };
    (matches!(constructor, "new" | "value")
        && ctx.identities().is_some_and(|index| {
            index.is_package_declaration(&identity, "flutter_bloc", "BlocProvider")
        }))
    .then_some(args)
}
