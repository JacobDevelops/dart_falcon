//! Reports inline framework groups without any test registrations.
//!
//! `include-methods` (array of strings, default: []) adds custom test registration helpers.

use crate::lint::correctness::test_assertions;
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
pub struct AvoidEmptyTestGroups;

impl Rule for AvoidEmptyTestGroups {
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
            include_methods: &crate::lint::options::get::<Options>(ctx, Self::NAME).include_methods,
            groups: Vec::new(),
            tests: Vec::new(),
            unknown_calls: Vec::new(),
            lifecycle_callbacks: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    groups: Vec<(Span, Span)>,
    tests: Vec<Span>,
    unknown_calls: Vec<Span>,
    include_methods: &'a [String],
    lifecycle_callbacks: Vec<Span>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        if let Expr::Ident(name) = call.callee().unparenthesized()
            && self.include_methods.iter().any(|method| method == name.name())
        {
            self.tests.push(call.span());
            return;
        }
        match test_assertions::package_call(call, state) {
            Some((name, package)) if test_assertions::is_framework(package) => {
                match name {
                    "test" | "testWidgets" => self.tests.push(call.span()),
                    "group" => {
                        // A nested group is itself a registration: only its
                        // empty leaf, not the containing suite, is reported.
                        self.tests.push(call.span());
                        if let Some(Expr::FuncExpr(callback)) =
                            call.args().positional().nth(1).map(Expr::unparenthesized)
                        {
                            self.groups.push((call.callee().span(), callback.span()));
                        } else {
                            self.unknown_calls.push(call.span());
                        }
                    }
                    "setUp" | "tearDown" | "setUpAll" | "tearDownAll" => {
                        if let Some(callback) = call.args().positional().next() {
                            self.lifecycle_callbacks.push(callback.span());
                        }
                    }
                    _ => self.unknown_calls.push(call.span()),
                }
            }
            _ => self.unknown_calls.push(call.span()),
        }
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.groups
            .iter()
            .filter(|(_, body)| {
                !self.tests.iter().any(|test| contains(*body, *test))
                    && !self.unknown_calls.iter().any(|call| {
                        contains(*body, *call)
                            && !self
                                .lifecycle_callbacks
                                .iter()
                                .any(|callback| contains(*callback, *call))
                    })
            })
            .map(|(span, _)| {
                self.ctx.diagnostic("Add a test to this group or remove the empty group.", *span)
            })
            .collect()
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
