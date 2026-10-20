//! Value providers borrow existing instances; newly constructed values need create.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::{
    constructor_span, scoped_constructor_invocation, value_name_is_shadowed,
};

#[derive(Default)]
pub struct AvoidInstantiatingInValueProvider;

impl Rule for AvoidInstantiatingInValueProvider {
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

// Generic named constructors wrap their receiver in GenericInstantiation.
fn constructor<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'_>,
) -> Option<(DeclarationIdentity, &'a str, ArgList<'a>)> {
    if let Expr::Call(call) = expression.unparenthesized() {
        let segments = falcon_analyze::expression_segments(call.callee(), true)?;
        if value_name_is_shadowed(segments.first()?, state) {
            return None;
        }
        let (identity, name) =
            falcon_analyze::resolve_call_constructor(call.callee(), &state.model, true)?;
        return Some((state.model.unaliased(identity), name, call.args()));
    }
    scoped_constructor_invocation(expression, state)
}

fn instantiates(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::Cascade(cascade) => instantiates(cascade.object(), state),
        Expr::Conditional(conditional) => {
            instantiates(conditional.then_expr(), state)
                || instantiates(conditional.else_expr(), state)
        }
        Expr::Binary(binary) if binary.op() == BinaryOp::NullCoalesce => {
            instantiates(binary.left(), state) || instantiates(binary.right(), state)
        }
        expression => constructor(expression, state).is_some_and(|(identity, constructor, _)| {
            state.signatures.has_constructor(&identity, constructor)
        }),
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, name, args)) = constructor(expression, state) else {
            return;
        };
        if name != "value"
            || !state.signatures.has_constructor(&identity, name)
            || !matches!(
                identity.name(),
                "Provider" | "ChangeNotifierProvider" | "ListenableProvider"
            )
            || !(identity.is_package("provider", identity.name())
                || state
                    .model
                    .declaration_uri(&identity)
                    .is_some_and(|uri| uri.starts_with("package:provider/")))
        {
            return;
        }
        let Some(value) = args.named().find(|arg| arg.name().name() == "value") else {
            return;
        };
        if instantiates(value.value(), state) {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-instantiating-in-value-provider",
                "Use create to construct a new provided instance instead of a value provider.",
                constructor_span(expression),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
