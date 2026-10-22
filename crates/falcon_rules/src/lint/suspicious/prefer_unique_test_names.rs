//! Reports duplicate test descriptions within a group and constant names in loops.
//!
//! `include-methods` (array of strings, default: []) adds custom test registration helpers.

use crate::lint::support::test_assertions;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Additional method names recognized as test registrations inside a group.
    include_methods: Vec<String>,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct PreferUniqueTestNames;

impl Rule for PreferUniqueTestNames {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !test_assertions::enabled(program, ctx) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            groups: Vec::new(),
            tests: Vec::new(),
            loops: Vec::new(),
            include_methods: &crate::lint::options::get::<Options>(ctx, Self::NAME).include_methods,
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    groups: Vec<Span>,
    tests: Vec<(String, Span, Option<Span>)>,
    loops: Vec<Span>,
    include_methods: &'a [String],
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        if matches!(node, Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_)) {
            self.loops.push(node.span());
        }
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let canonical = test_assertions::package_call(call, state)
            .filter(|(_, package)| test_assertions::is_framework(package));
        if canonical.is_some_and(|(name, _)| name == "group")
            && let Some(Expr::FuncExpr(callback)) =
                call.args().positional().nth(1).map(Expr::unparenthesized)
        {
            self.groups.push(callback.span());
        }
        let configured = matches!(call.callee().unparenthesized(), Expr::Ident(name) if self.include_methods.iter().any(|method| method == name.name()));
        if (configured || canonical.is_some_and(|(name, _)| matches!(name, "test" | "testWidgets")))
            && let Some(description) = call.args().positional().next()
            && let Some(falcon_analyze::ConstantValue::String(value)) =
                falcon_analyze::evaluate_constant_with_local_values(
                    description,
                    state.constant_owner.as_ref(),
                    &state.visible_local_constants(),
                    &state.model,
                    &state.signatures,
                )
        {
            let function = call
                .syntax()
                .ancestors()
                .find(|ancestor| {
                    FunctionDecl::cast(*ancestor).is_some()
                        || MethodDecl::cast(*ancestor).is_some()
                        || ConstructorDecl::cast(*ancestor).is_some()
                        || GetterDecl::cast(*ancestor).is_some()
                        || SetterDecl::cast(*ancestor).is_some()
                        || matches!(Expr::cast(*ancestor), Some(Expr::FuncExpr(_)))
                        || matches!(Stmt::cast(*ancestor), Some(Stmt::LocalFunc(_)))
                })
                .map(|function| function.span());
            self.tests.push((value, description.span(), function));
        }
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let scope = |span: Span| {
            self.groups
                .iter()
                .filter(|group| group.contains(span))
                .min_by_key(|group| group.end - group.start)
                .copied()
        };
        let mut diagnostics = Vec::new();
        for (index, (name, span, function)) in self.tests.iter().enumerate() {
            let repeated_by_loop = self.loops.iter().any(|body| {
                body.contains(*span)
                    && scope(*span)
                        .is_none_or(|group| group.contains(*body))
                    && function.is_none_or(|function| {
                        function.contains(*body)
                    })
            });
            if repeated_by_loop
                || self.tests[..index]
                    .iter()
                    .any(|(previous, location, previous_function)| {
                        previous == name
                            && previous_function == function
                            && scope(*location) == scope(*span)
                    })
            {
                diagnostics.push(self.ctx.diagnostic("Use a unique test description within this group.", *span));
            }
        }
        diagnostics
    }
}
