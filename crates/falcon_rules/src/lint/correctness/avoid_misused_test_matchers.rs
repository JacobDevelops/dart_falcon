//! Warns when a test matcher cannot apply to the actual value's known static type.
//!
//! Checks collection, string, numeric, boolean, asynchronous and function matchers,
//! including composed matchers and `isIn` membership checks. Known sets are
//! distinguished from lists, while an unknown Iterable implementation may be a
//! list. Unknown types and unresolved matcher identities are left alone.

use falcon_analyze::{
    AnalyzeContext, QualifiedName, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
    SemanticRuleVisitor, SemanticState, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::support::test_assertions;

#[derive(Default)]
pub struct AvoidMisusedTestMatchers;

impl Rule for AvoidMisusedTestMatchers {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !test_assertions::enabled(program, ctx) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
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
            || !test_assertions::MATCHER_PACKAGES.contains(&package)
        {
            return;
        }
        let mut arguments = call.args().positional();
        let (Some(actual), Some(matcher)) = (arguments.next(), arguments.next()) else {
            return;
        };
        let ty = state.infer(actual);
        if incompatible(matcher, &ty, state, self.ctx, 0) {
            self.diags.push(self.ctx.diagnostic(
                "Use a matcher compatible with the actual value's static type.",
                matcher.span(),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Unknown,
    Null,
    Bool,
    Number,
    String,
    List,
    Set,
    Iterable,
    Map,
    Future,
    Stream,
    Function,
    Record,
}

fn kind(ty: &ResolvedType, state: &SemanticState<'_>) -> Kind {
    match ty.view() {
        ResolvedTypeView::Null => return Kind::Null,
        ResolvedTypeView::Function { .. } => return Kind::Function,
        ResolvedTypeView::Record { .. } => return Kind::Record,
        ResolvedTypeView::Interface {
            extension_type: false,
            ..
        } => {}
        _ => return Kind::Unknown,
    }
    for (library, name, kind) in [
        ("dart:core", "bool", Kind::Bool),
        ("dart:core", "num", Kind::Number),
        ("dart:core", "String", Kind::String),
        ("dart:core", "List", Kind::List),
        ("dart:core", "Set", Kind::Set),
        ("dart:core", "Iterable", Kind::Iterable),
        ("dart:core", "Map", Kind::Map),
        ("dart:async", "Future", Kind::Future),
        ("dart:async", "Stream", Kind::Stream),
        ("dart:core", "Function", Kind::Function),
    ] {
        if state.signatures.implements_sdk(ty, library, name) == Truth::Yes {
            return kind;
        }
    }
    Kind::Unknown
}

fn matcher_name<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'a>,
) -> Option<&'a str> {
    let expression = match expression.unparenthesized() {
        Expr::Call(call) => call.callee(),
        other => other,
    };
    let (head, member) = crate::lint::expressions::identifier_name(expression.unparenthesized())?;
    let name = member.unwrap_or(head);
    let prefix = member.map(|_| head);
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
    test_assertions::MATCHER_PACKAGES
        .iter()
        .any(|package| {
            ctx.identities()
                .is_some_and(|index| index.is_package_declaration(&identity, package, name))
        })
        .then_some(name)
}

fn incompatible<'a>(
    expression: Expr<'a>,
    ty: &ResolvedType,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'a>,
    depth: usize,
) -> bool {
    if depth >= 64 {
        return false;
    }
    let expression = expression.unparenthesized();
    let Some(name) = matcher_name(expression, state, ctx) else {
        return incompatible_literal(expression, ty, state);
    };
    if matches!(name, "isNull" | "isNotNull") {
        return state.signatures.is_non_nullable(ty);
    }
    if let Expr::Call(call) = expression {
        if name == "isIn" {
            return call.args().positional().next().is_some_and(|values| {
                state
                    .signatures
                    .instantiated_supertype(
                        &state.infer(values),
                        "dart:core",
                        "Iterable",
                        &state.model,
                    )
                    .and_then(|iterable| iterable.type_argument(0))
                    .is_some_and(|element| {
                        state.signatures.relation(
                            ty,
                            TypeRelation::RuntimeUnrelatedTo(&element),
                            &state.model,
                        ) == Truth::Yes
                    })
            });
        }
        if name == "isA" {
            return call.type_args().next().is_some_and(|argument| {
                let expected = state
                    .model
                    .resolve_type_in(argument, &state.type_parameters);
                let actual_kind = kind(ty, state);
                let expected_kind = kind(&expected, state);
                if !matches!(
                    actual_kind,
                    Kind::Bool | Kind::Number | Kind::String | Kind::Null
                ) && !matches!(
                    expected_kind,
                    Kind::Bool | Kind::Number | Kind::String | Kind::Null
                ) {
                    return false;
                }
                state.signatures.relation(
                    ty,
                    TypeRelation::RuntimeUnrelatedTo(&expected),
                    &state.model,
                ) == Truth::Yes
            });
        }
        if matches!(name, "isNot" | "equals") {
            return call
                .args()
                .positional()
                .next()
                .is_some_and(|inner| incompatible(inner, ty, state, ctx, depth + 1));
        }
        if matches!(name, "allOf" | "anyOf") {
            let mut args = call.args().positional().peekable();
            if args.peek().is_none() {
                return false;
            }
            let mut invalid =
                |inner| incompatible_composite(inner, ty, state, ctx, depth + 1, name == "allOf");
            return if name == "allOf" {
                args.any(&mut invalid)
            } else {
                args.all(&mut invalid)
            };
        }
    }
    let actual = kind(ty, state);
    incompatible_kind(name, actual)
}

fn incompatible_literal<'a>(
    expression: Expr<'a>,
    ty: &ResolvedType,
    state: &SemanticState<'a>,
) -> bool {
    match expression {
        Expr::NullLit(_) => state.signatures.is_non_nullable(ty),
        Expr::BoolLit(_) | Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::StringLit(_) => {
            state.signatures.relation(
                ty,
                TypeRelation::RuntimeUnrelatedTo(&state.infer(expression)),
                &state.model,
            ) == Truth::Yes
        }
        _ => false,
    }
}

fn incompatible_composite<'a>(
    expression: Expr<'a>,
    ty: &ResolvedType,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'a>,
    depth: usize,
    all: bool,
) -> bool {
    let elements = match expression.unparenthesized() {
        Expr::List(list) => list.elements(),
        Expr::Set(set) => set.elements(),
        _ => return incompatible(expression, ty, state, ctx, depth),
    };
    let mut elements = elements.peekable();
    if elements.peek().is_none() {
        return false;
    }
    let mut invalid = |element| {
        matches!(element, CollectionElement::Expr(inner)
            if incompatible(inner, ty, state, ctx, depth))
    };
    if all {
        elements.any(&mut invalid)
    } else {
        elements.all(&mut invalid)
    }
}

fn incompatible_kind(name: &str, actual: Kind) -> bool {
    if actual == Kind::Unknown {
        return false;
    }
    match name {
        "isTrue" | "isFalse" => actual != Kind::Bool,
        "isZero"
        | "isNonZero"
        | "isPositive"
        | "isNonPositive"
        | "isNegative"
        | "isNonNegative"
        | "closeTo"
        | "inInclusiveRange"
        | "inExclusiveRange"
        | "inOpenClosedRange"
        | "inClosedOpenRange"
        | "greaterThan"
        | "greaterThanOrEqualTo"
        | "lessThan"
        | "lessThanOrEqualTo" => actual != Kind::Number,
        "isEmpty" | "isNotEmpty" | "hasLength" | "contains" => !matches!(
            actual,
            Kind::String | Kind::List | Kind::Set | Kind::Iterable | Kind::Map
        ),
        "isMap" | "containsValue" | "containsPair" => actual != Kind::Map,
        "isList" => !matches!(actual, Kind::List | Kind::Iterable),
        "everyElement" | "anyElement" | "orderedEquals" | "unorderedEquals"
        | "unorderedMatches" | "pairwiseCompare" | "containsAll" | "containsAllInOrder"
        | "containsOnce" => !matches!(actual, Kind::List | Kind::Set | Kind::Iterable),
        "equalsIgnoringCase"
        | "equalsIgnoringWhitespace"
        | "startsWith"
        | "endsWith"
        | "stringContainsInOrder"
        | "matches" => actual != Kind::String,
        "returnsNormally" | "prints" => actual != Kind::Function,
        "completes" | "doesNotComplete" => actual != Kind::Future,
        "emitsDone" | "emits" | "emitsError" | "mayEmit" | "emitsAnyOf" | "emitsInOrder"
        | "emitsThrough" | "mayEmitMultiple" | "neverEmits" | "emitsInAnyOrder" => {
            actual != Kind::Stream
        }
        "throwsA"
        | "throwsException"
        | "throwsStateError"
        | "throwsUnimplementedError"
        | "throwsArgumentError"
        | "throwsFormatException"
        | "throwsRangeError"
        | "throwsUnsupportedError"
        | "throwsNoSuchMethodError"
        | "throwsConcurrentModificationError" => !matches!(actual, Kind::Future | Kind::Function),
        _ => false,
    }
}
