//! Recommends Matcher expectations and direct type, equality, or ordering matchers.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, QualifiedName, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::correctness::test_assertions;

#[derive(Default)]
pub struct PreferTestMatchers;

impl Rule for PreferTestMatchers {
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
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else { return };
        let Some((name, package)) = test_assertions::package_call(call, state) else {
            return;
        };
        if !matches!(name, "expect" | "expectLater")
            || !(test_assertions::is_framework(package) || package == "matcher")
        {
            return;
        }
        let mut args = call.args().positional();
        let (Some(actual), Some(expected)) = (args.next(), args.next()) else {
            return;
        };
        let boolean = boolean_expectation(expected, state, self.ctx);
        if boolean.is_some() && matches!(actual.unparenthesized(), Expr::Is(_)) {
            self.diags.push(self.ctx.diagnostic(
                "prefer-test-matchers",
                "Use isA or isNot(isA) instead of testing the type before expect.",
                actual.span(),
            ));
            return;
        }
        if boolean.is_some()
            && let Expr::Binary(binary) = actual.unparenthesized()
            && matches!(
                binary.op(),
                BinaryOp::EqEq
                    | BinaryOp::NotEq
                    | BinaryOp::Lt
                    | BinaryOp::LtEq
                    | BinaryOp::Gt
                    | BinaryOp::GtEq
            )
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-test-matchers",
                "Match the actual value directly with equals, isNot, or an ordering matcher instead of comparing it before expect.",
                actual.span(),
            ));
            return;
        }
        let message = match expected.unparenthesized() {
            Expr::BoolLit(value) => Some(if value.value() {
                "Use isTrue instead of the literal true matcher."
            } else {
                "Use isFalse instead of the literal false matcher."
            }),
            Expr::NullLit(_) => Some("Use isNull instead of the literal null matcher."),
            _ if known_non_matcher(&state.infer(expected), state, self.ctx) => {
                Some("Wrap the expected value in a test matcher such as equals.")
            }
            _ => None,
        };
        if let Some(message) = message {
            self.diags.push(
                self.ctx
                    .diagnostic("prefer-test-matchers", message, expected.span()),
            );
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}

fn boolean_expectation(
    expression: Expr<'_>,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> Option<bool> {
    if let Expr::BoolLit(value) = expression.unparenthesized() {
        return Some(value.value());
    }
    let (head, member) = crate::lint::expressions::identifier_name(expression.unparenthesized())?;
    let name = member.unwrap_or(head);
    let prefix = member.map(|_| head);
    let value = match name {
        "isTrue" => true,
        "isFalse" => false,
        _ => return None,
    };
    if state.environment.is_bound(prefix.unwrap_or(name))
        || (prefix.is_none()
            && state.enclosing_type.as_ref().is_some_and(|receiver| {
                state
                    .signatures
                    .resolved_member(receiver, name, &state.model)
                    .is_some()
            }))
    {
        return None;
    }
    let qualified = prefix.map_or_else(
        || QualifiedName::from(name),
        |prefix| QualifiedName::prefixed(prefix, name),
    );
    let identity = state.model.resolve_value(qualified)?;
    ["test", "test_core", "test_api", "flutter_test", "matcher"]
        .iter()
        .any(|package| {
            ctx.identities
                .is_some_and(|index| index.is_package_declaration(&identity, package, name))
        })
        .then_some(value)
}

// Unknown and dynamic types may hold a Matcher. Only types known not to be
// Matcher implementations are diagnosed, rather than treating failed resolution
// or an incomplete external superclass graph as proof of a violation.
fn known_non_matcher(
    ty: &falcon_analyze::ResolvedType,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    use falcon_analyze::ResolvedType;
    match ty {
        ResolvedType::Null | ResolvedType::Record { .. } => true,
        ResolvedType::Interface { identity, .. } if identity.is_sdk("dart:core", "Object") => false,
        ResolvedType::Interface { identity, .. } => {
            state.signatures.supertype_matches(identity, &|identity| {
                ["matcher", "test", "test_core", "test_api", "flutter_test"]
                    .iter()
                    .any(|package| {
                        ctx.identities.is_some_and(|index| {
                            index.is_package_declaration(identity, package, "Matcher")
                        })
                    })
            }) == falcon_analyze::Truth::No
        }
        _ => false,
    }
}
