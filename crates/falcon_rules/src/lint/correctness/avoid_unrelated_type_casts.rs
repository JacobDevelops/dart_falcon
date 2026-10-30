//! Flags casts between types proven unrelated by canonical type resolution.
//!
//! Checks `as` expressions, cast patterns, and SDK collection `cast<T>()`
//! element/key/value conversions. Dynamic, unresolved, and potentially related
//! types are not reported. Collection casts are lazy but fail when an element
//! of the incompatible type is read.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, ResolvedTypeView, Rule, RulePreparation, Truth, TypeRelation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnrelatedTypeCasts;

impl Rule for AvoidUnrelatedTypeCasts {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn unrelated(
        source: &ResolvedType,
        target: &ResolvedType,
        state: &SemanticState<'_>,
    ) -> bool {
        // Object can hold structural values, and a type parameter may be
        // instantiated with one even when its bound is not structural.
        if source.interface("dart:core", "Object")
            || target.interface("dart:core", "Object")
            || matches!(source.view(),
                ResolvedTypeView::TypeParameter { .. } | ResolvedTypeView::NonNullTypeParameter { .. }
            )
            || matches!(target.view(),
                ResolvedTypeView::TypeParameter { .. } | ResolvedTypeView::NonNullTypeParameter { .. }
            )
        {
            return false;
        }
        state
            .signatures
            .relation(source, TypeRelation::RuntimeUnrelatedTo(target), &state.model)
            == Truth::Yes
    }

    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic("Cast types are unrelated — use a compatible type instead.", span));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        match node {
            Expr::As(cast) => {
                let target = state
                    .model
                    .resolve_type_in(cast.dart_type(), &state.type_parameters);
                if Self::unrelated(&state.infer(cast.expr()), &target, state) {
                    self.report(cast.span());
                }
            }
            Expr::Call(call) => {
                let Expr::Field(field) = call.callee().unparenthesized() else {
                    return;
                };
                if field.field().name() != "cast"
                    || !call.args().positional().is_empty()
                    || !call.args().named().is_empty()
                {
                    return;
                }
                let receiver = state.infer(field.object());
                let sdk_collection = ["Iterable", "List", "Set", "Map"]
                    .iter()
                    .any(|name| receiver.interface("dart:core", name));
                if !sdk_collection {
                    return;
                }
                let map = state.signatures.instantiated_supertype(
                    &receiver,
                    "dart:core",
                    "Map",
                    &state.model,
                );
                let source = map.or_else(|| {
                    state.signatures.instantiated_supertype(
                        &receiver,
                        "dart:core",
                        "Iterable",
                        &state.model,
                    )
                });
                let Some(source) = source else {
                    return;
                };
                for (index, target) in call.type_args().enumerate() {
                    let Some(element) = source.type_argument(index) else {
                        continue;
                    };
                    let target = state.model.resolve_type_in(target, &state.type_parameters);
                    if Self::unrelated(&element, &target, state) {
                        self.report(call.span());
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if let Pattern::Cast(cast) = node
            && let Some(source) = matched
        {
            let target = state
                .model
                .resolve_type_in(cast.cast_type(), &state.type_parameters);
            if Self::unrelated(source, &target, state) {
                self.report(cast.span());
            }
        }
    }
}
