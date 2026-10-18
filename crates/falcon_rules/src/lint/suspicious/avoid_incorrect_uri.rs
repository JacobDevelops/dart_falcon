//! Reject runtime-invalid Uri constructor arguments: conflicting path inputs and
//! query parameter values that are neither null, strings nor iterables of strings.
//! Only resolved dart:core Uri constructors are checked; unknown values are skipped.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
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

impl Collector<'_> {
    fn report(&mut self, span: Span, message: &str) {
        self.diags
            .push(self.ctx.diagnostic("avoid-incorrect-uri", message, span));
    }

    fn query<'a>(&mut self, value: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Map(map) = value.unparenthesized() {
            for element in map.elements() {
                if let MapElement::Entry(entry) = element
                    && invalid_query_value(entry.value(), state)
                {
                    self.report(
                        entry.value().span(),
                        "Uri query parameter values must be null, strings or iterables of strings.",
                    );
                }
            }
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
        _ => matches!(state.infer(value), ResolvedType::Interface { identity, nullable: false, .. }
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
                let path = args.named().find(|arg| arg.name().name() == "path");
                let segments = args.named().find(|arg| arg.name().name() == "pathSegments");
                if let (Some(path), Some(segments)) = (path, segments)
                    && matches!(
                        state.infer(path.value()),
                        ResolvedType::Interface {
                            nullable: false,
                            ..
                        }
                    )
                    && matches!(
                        state.infer(segments.value()),
                        ResolvedType::Interface {
                            nullable: false,
                            ..
                        }
                    )
                {
                    self.report(
                        segments.span(),
                        "Do not provide both path and pathSegments to Uri.",
                    );
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
