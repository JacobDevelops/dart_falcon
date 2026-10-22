//! Reports repeated identical, side-effect-free assertions in a straight-line block.
//!
//! Only `_test.dart` files importing `test` or `flutter_test` are inspected.
//! Calls must resolve to the test framework or matcher package. Comparisons use
//! the actual and matcher source spelling. Literal/local actual values, standard
//! matcher constants and simple framework matcher factories are eligible.
//! Unknown local matcher objects and asynchronous local actuals are excluded.
//! Intervening non-assertion statements, branches, effectful arguments, awaited
//! calls, and chained calls end the comparison run.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use std::collections::BTreeMap;

use crate::lint::support::test_assertions;

#[derive(Default)]
pub struct AvoidDuplicateTestAssertions;

impl Rule for AvoidDuplicateTestAssertions {
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
            assertions: BTreeMap::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    assertions: BTreeMap<(usize, usize), (&'a str, &'a str, &'a str)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Some((name, package)) = test_assertions::package_call(call, state) else {
            return;
        };
        if !matches!(name, "expect" | "expectLater")
            || !test_assertions::is_assertion(name, package)
        {
            return;
        }
        let mut args = call.args().positional();
        let (Some(actual), Some(matcher)) = (args.next(), args.next()) else {
            return;
        };
        if args.next().is_some()
            || !call.args().named().is_empty()
            || !inert(actual, state)
            || !inert_matcher(matcher, state)
            || (name == "expectLater" && matches!(actual.unparenthesized(), Expr::Ident(_)))
        {
            return;
        }
        let actual = actual.span();
        let matcher = matcher.span();
        self.assertions.insert(
            (call.span().start, call.span().end),
            (
                name,
                &self.ctx.source()[actual.start..actual.end],
                &self.ctx.source()[matcher.start..matcher.end],
            ),
        );
    }

    fn visit_block_end(&mut self, block: Block<'a>, _state: &SemanticState<'a>) {
        let mut seen = Vec::with_capacity(block.stmts().len());
        for statement in block.stmts() {
            let assertion = match statement {
                Stmt::Expr(statement) => {
                    let span = statement.expr().span();
                    self.assertions.get(&(span.start, span.end))
                }
                _ => None,
            };
            let Some(&key) = assertion else {
                seen.clear();
                continue;
            };
            if seen.contains(&key) {
                self.diagnostics.push(self.ctx.diagnostic("This assertion is repeated; remove the duplicate assertion.", statement.span()));
            } else {
                seen.push(key);
            }
        }
    }
}

fn inert(expression: Expr<'_>, state: &SemanticState<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) => true,
        Expr::StringLit(string) => string.interpolations().next().is_none(),
        Expr::Ident(name) => {
            if state.environment.is_bound(name.name()) && !state.is_own_field(name.name()) {
                return true;
            }
            matches!(
                name.name(),
                "isTrue"
                    | "isFalse"
                    | "isNull"
                    | "isNotNull"
                    | "isEmpty"
                    | "isNotEmpty"
                    | "anything"
            ) && state
                .model
                .resolve_value(falcon_analyze::QualifiedName::from(name.name()))
                .is_some_and(|identity| {
                    crate::lint::support::test_assertions::MATCHER_PACKAGES
                        .iter()
                        .any(|package| identity.is_package(package, name.name()))
                })
        }
        _ => false,
    }
}

fn inert_matcher<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    if let Expr::Ident(name) = expression.unparenthesized()
        && state.environment.is_bound(name.name())
    {
        return false;
    }
    if inert(expression, state) {
        return true;
    }
    let Expr::Call(call) = expression.unparenthesized() else {
        return false;
    };
    let Some((name, package)) = test_assertions::package_call(call, state) else {
        return false;
    };
    matches!(
        name,
        "equals"
            | "same"
            | "greaterThan"
            | "lessThan"
            | "greaterThanOrEqualTo"
            | "lessThanOrEqualTo"
    ) && (test_assertions::is_framework(package) || package == "matcher")
        && call.args().named().is_empty()
        && call
            .args()
            .positional()
            .all(|argument| inert(argument, state))
}
