//! Reports inline test callbacks that contain no recognized assertion calls.
//!
//! Only `_test.dart` files importing `test` or `flutter_test` are inspected,
//! and `test`/`testWidgets` must resolve to a framework package. Assertions
//! include framework expectations, asynchronous expectation wrappers, `fail`,
//! and mockito/mocktail verification calls. Assertions in unused local closures
//! or functions and nested tests do not satisfy the enclosing test.
//!
//! ## Options
//!
//! `include_assertions` is a list of additional assertion function names. These
//! names explicitly trust project helpers, including local functions; prefixed
//! names can be supplied as written at their call sites.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::test_assertions;
use crate::lint::options;

#[derive(Default)]
pub struct MissingTestAssertion;

impl Rule for MissingTestAssertion {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !test_assertions::enabled(program, ctx) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let additional = options::strings(
            options::for_rule(ctx, "missing-test-assertion"),
            "include_assertions",
        )
        .into_iter()
        .flatten()
        .collect();
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            additional,
            tests: Vec::new(),
            closures: Vec::new(),
            references: Vec::new(),
            assertions: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    additional: Vec<&'a str>,
    tests: Vec<(Span, Span)>,
    closures: Vec<(Span, Span)>,
    references: Vec<(Span, Span)>,
    assertions: Vec<Span>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.tests
            .iter()
            .filter(|(_, callback)| {
                !self.assertions.iter().any(|assertion| {
                    contains(*callback, *assertion)
                        && !self.tests.iter().any(|(_, nested)| {
                            *nested != *callback
                                && contains(*callback, *nested)
                                && contains(*nested, *assertion)
                        })
                        && !self.closures.iter().any(|(binding, nested)| {
                            contains(*callback, *nested)
                                && contains(*nested, *assertion)
                                && !self.references.iter().any(|(referenced, usage)| {
                                    referenced == binding
                                        && contains(*callback, *usage)
                                        && !contains(*nested, *usage)
                                })
                        })
                })
            })
            .map(|(call, _)| {
                self.ctx.diagnostic(
                    "missing-test-assertion",
                    "This test has no assertion; add an expectation or verification.",
                    *call,
                )
            })
            .collect()
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        match node {
            Stmt::LocalFunc(function) => self
                .closures
                .push((function.name().span(), function.span())),
            Stmt::LocalVar(local) => {
                for declaration in local.declarators() {
                    if let Some(Expr::FuncExpr(function)) =
                        declaration.initializer().map(Expr::unparenthesized)
                    {
                        self.closures
                            .push((declaration.name().span(), function.span()));
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Ident(name) = node
            && let Some(binding) = state.environment.binding_declaration(name.name())
        {
            self.references.push((binding, name.span()));
        }
        let Expr::Call(call) = node else {
            return;
        };
        let callee = call.callee().span();
        let spelling = &self.ctx.source()[callee.start..callee.end];
        if self.additional.contains(&spelling) {
            self.assertions.push(call.span());
        }
        let Some((name, package)) = test_assertions::package_call(call, state) else {
            return;
        };
        if test_assertions::is_assertion(name, package) {
            self.assertions.push(call.span());
        }
        if matches!(name, "test" | "testWidgets")
            && test_assertions::is_framework(package)
            && let Some(Expr::FuncExpr(callback)) =
                call.args().positional().nth(1).map(Expr::unparenthesized)
        {
            self.tests.push((call.callee().span(), callback.span()));
        }
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
