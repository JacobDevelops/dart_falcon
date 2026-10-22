//! Reject runtime-invalid Uri constructor arguments: conflicting path or query inputs and
//! query parameter values that are neither null, strings nor iterables of strings.
//! Only resolved dart:core Uri constructors are checked; unknown values are skipped.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedTypeView, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter::scoped_constructor_invocation;

#[derive(Default)]
pub struct AvoidIncorrectUri;

impl Rule for AvoidIncorrectUri {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span, message: &str) {
        self.diags
            .push(self.ctx.diagnostic(message, span));
    }

    fn query<'a>(&mut self, value: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Map(map) = value.unparenthesized() {
            for element in map.elements() {
                self.query_element(element, state);
            }
        }
    }

    fn query_element<'a>(&mut self, element: MapElement<'a>, state: &SemanticState<'a>) {
        match element {
            MapElement::Entry(entry) if invalid_query_value(entry.value(), state) => self.report(
                entry.value().span(),
                "`Uri` query parameter values must be `null`, strings or iterables of strings.",
            ),
            MapElement::If(element) => {
                self.query_element(element.then_entry(), state);
                if let Some(otherwise) = element.else_entry() {
                    self.query_element(otherwise, state);
                }
            }
            MapElement::For(element) => self.query_element(element.entry(), state),
            MapElement::Entry(_) | MapElement::Spread(_) => {}
        }
    }
}

fn invalid_query_value<'a>(value: Expr<'a>, state: &SemanticState<'a>) -> bool {
    match value.unparenthesized() {
        Expr::NullLit(_) | Expr::StringLit(_) => false,
        Expr::List(list) => list.elements().any(|element| {
            matches!(element, CollectionElement::Expr(value) if invalid_string(value, state))
        }),
        Expr::Set(set) => set.elements().any(|element| {
            matches!(element, CollectionElement::Expr(value) if invalid_string(value, state))
        }),
        _ => invalid_string(value, state),
    }
}

fn invalid_string<'a>(value: Expr<'a>, state: &SemanticState<'a>) -> bool {
    match value.unparenthesized() {
        Expr::NullLit(_)
        | Expr::IntLit(_)
        | Expr::DoubleLit(_)
        | Expr::BoolLit(_)
        | Expr::SymbolLit(_)
        | Expr::Map(_)
        | Expr::Record(_)
        | Expr::FuncExpr(_)
        | Expr::List(_)
        | Expr::Set(_) => true,
        _ => matches!(state.infer(value).view(), ResolvedTypeView::Interface { identity, nullable: false, .. }
            if ["int", "double", "num", "bool", "Symbol", "Uri", "DateTime", "Duration", "Map"]
                .iter().any(|name| identity.is_sdk("dart:core", name))),
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, constructor, args)) = scoped_constructor_invocation(node, state) else {
            return;
        };
        if !identity.is_sdk("dart:core", "Uri") {
            return;
        }
        match constructor {
            "new" => {
                for (first, second, message) in [
                    (
                        "path",
                        "pathSegments",
                        "Do not provide both `path` and `pathSegments` to `Uri`.",
                    ),
                    (
                        "query",
                        "queryParameters",
                        "Do not provide both `query` and `queryParameters` to `Uri`.",
                    ),
                ] {
                    let first = args.named().find(|arg| arg.name().name() == first);
                    let second = args.named().find(|arg| arg.name().name() == second);
                    if let (Some(first), Some(second)) = (first, second)
                        && !(first.name().name() == "query"
                            && matches!(first.value().unparenthesized(), Expr::StringLit(literal)
                                if literal.interpolations().next().is_none() && literal.value().is_empty()))
                        && matches!(
                            state.infer(first.value()).view(),
                            ResolvedTypeView::Interface {
                                nullable: false,
                                ..
                            }
                        )
                        && matches!(
                            state.infer(second.value()).view(),
                            ResolvedTypeView::Interface {
                                nullable: false,
                                ..
                            }
                        )
                    {
                        self.report(second.span(), message);
                    }
                }
                if let Some(query) = args
                    .named()
                    .find(|arg| arg.name().name() == "queryParameters")
                {
                    self.query(query.value(), state);
                }
            }
            "http" | "https" => {
                if let Some(query) = args.positional().nth(2) {
                    self.query(query, state);
                }
            }
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
