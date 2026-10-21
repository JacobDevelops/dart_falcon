//! Shared semantic collection and conservative argument checks for widget replacements.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

pub(super) fn prepare<'a>(
    ctx: &'a AnalyzeContext<'a>,
    widget: &'static str,
    message: &'static str,
    matches: fn(ArgList<'_>, &SemanticState<'_>, &AnalyzeContext<'_>) -> bool,
) -> RulePreparation<'a> {
    RulePreparation::Semantic(Box::new(Collector {
        ctx,
        widget,
        message,
        matches,
        diagnostics: Vec::new(),
    }))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    widget: &'static str,
    message: &'static str,
    matches: fn(ArgList<'_>, &SemanticState<'_>, &AnalyzeContext<'_>) -> bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let args = match node {
            Expr::Call(call) => call.args(),
            Expr::New(construction) => construction.args(),
            _ => return,
        };
        if !(self.matches)(args, state, self.ctx) {
            return;
        }
        if self.widget == "Container"
            && let Some(ResolvedType::Interface { identity, .. }) = state.expected()
            && self
                .ctx
                .identities()
                .is_some_and(|index| index.is_package_declaration(identity, "flutter", "Container"))
        {
            return;
        }
        if let Some((identity, "new", _)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
            && self.ctx.identities().is_some_and(|index| {
                index.is_package_declaration(&identity, "flutter", self.widget)
            })
        {
            self.diagnostics.push(
                self.ctx
                    .diagnostic(self.message, crate::lint::flutter::constructor_span(node)),
            );
        }
    }
}

pub(super) fn only_named(args: ArgList<'_>, allowed: &[&str]) -> bool {
    args.positional().is_empty()
        && args
            .named()
            .all(|argument| allowed.contains(&argument.name().name()))
}

pub(super) fn argument<'a>(args: ArgList<'a>, name: &str) -> Option<Expr<'a>> {
    args.named()
        .find(|argument| argument.name().name() == name)
        .map(|argument| argument.value())
}

pub(super) fn only_property(
    args: ArgList<'_>,
    property: &str,
    allowed: &[&str],
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    only_named(args, allowed)
        && ["child", property].into_iter().all(|name| {
            argument(args, name).is_some_and(|value| {
                crate::lint::semantic_type_operations::nullability(
                    &state.infer(value),
                    &state.signatures,
                    ctx.types(),
                ) == Truth::No
            })
        })
}
