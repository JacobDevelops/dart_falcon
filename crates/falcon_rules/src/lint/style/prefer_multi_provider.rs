//! Prefer a flat MultiProvider to directly nested provider widgets.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::{constructor_span, scoped_constructor_invocation};

#[derive(Default)]
pub struct PreferMultiProvider;

impl Rule for PreferMultiProvider {
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

fn provider_args<'a>(expression: Expr<'a>, state: &SemanticState<'_>) -> Option<ArgList<'a>> {
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
        _ => scoped_constructor_invocation(expression, state)?,
    };
    if !state.signatures.has_constructor(&identity, constructor) {
        return None;
    }
    if !matches!(
        identity.name(),
        "Provider"
            | "ChangeNotifierProvider"
            | "ListenableProvider"
            | "FutureProvider"
            | "StreamProvider"
            | "ProxyProvider"
            | "ProxyProvider0"
            | "ProxyProvider2"
            | "ProxyProvider3"
            | "ProxyProvider4"
            | "ProxyProvider5"
            | "ProxyProvider6"
            | "ChangeNotifierProxyProvider"
            | "ChangeNotifierProxyProvider0"
            | "ChangeNotifierProxyProvider2"
            | "ChangeNotifierProxyProvider3"
            | "ChangeNotifierProxyProvider4"
            | "ChangeNotifierProxyProvider5"
            | "ChangeNotifierProxyProvider6"
            | "ListenableProxyProvider"
            | "ListenableProxyProvider0"
            | "ListenableProxyProvider2"
            | "ListenableProxyProvider3"
            | "ListenableProxyProvider4"
            | "ListenableProxyProvider5"
            | "ListenableProxyProvider6"
    ) || !(identity.is_package("provider", identity.name())
        || state
            .model
            .declaration_uri(&identity)
            .is_some_and(|uri| uri.starts_with("package:provider/")))
    {
        return None;
    }
    Some(args)
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Some(args) = provider_args(expression, state) else {
            return;
        };
        let Some(child) = args.named().find(|arg| arg.name().name() == "child") else {
            return;
        };
        if provider_args(child.value(), state).is_some() {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-multi-provider",
                "Replace directly nested providers with MultiProvider.",
                constructor_span(expression),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
