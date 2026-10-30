//! Reports collection lookups whose argument type is unrelated to the
//! collection's type argument.
//!
//! Adopted from the official `collection_methods_unrelated_type` lint. With
//! exactly one argument, `Iterable<E>.contains`, `List<E>.remove`,
//! `Map<K, V>.containsKey`/`containsValue`/`remove`, `Queue<E>.remove`,
//! `Set<E>.lookup`/`remove`, and `Map<K, V>.[]` are checked on a receiver
//! (explicit, or the implicit `this` of the enclosing type) that implements the
//! declaring collection, whatever the receiver overrides. The argument must not
//! be unrelated ([`TypeRelation::RuntimeUnrelatedTo`]) to the type argument.
//!
//! [`TypeRelation::RuntimeUnrelatedTo`]: falcon_analyze::TypeRelation::RuntimeUnrelatedTo

use falcon_analyze::TypeKind;
use falcon_analyze::{AnalyzeContext, ResolvedType, ResolvedTypeView, Rule, RulePreparation, Truth, TypeRelation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct CollectionMethodsUnrelatedType;

impl Rule for CollectionMethodsUnrelatedType {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics) }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// `(method, library, collection, type argument index)` in the linter's order.
const METHODS: &[(&str, &str, &str, usize)] = &[
    ("contains", "dart:core", "Iterable", 0),
    ("remove", "dart:core", "List", 0),
    ("containsKey", "dart:core", "Map", 0),
    ("containsValue", "dart:core", "Map", 1),
    ("remove", "dart:core", "Map", 0),
    ("remove", "dart:collection", "Queue", 0),
    ("lookup", "dart:core", "Set", 0),
    ("remove", "dart:core", "Set", 0),
];

impl<'a> Collector<'a> {
    fn check(
        &mut self,
        argument: Expr<'a>,
        expected: &ResolvedType,
        context: Option<&ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        let actual = state.infer_with_context(argument, context);
        if state
            .signatures
            .relation(&actual, TypeRelation::RuntimeUnrelatedTo(expected), &state.model)
            != Truth::Yes
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("The argument type is unrelated to this collection's element type.", argument.span()));
    }

    fn expected_argument(
        receiver: &ResolvedType,
        method: &str,
        state: &SemanticState<'_>,
    ) -> Option<ResolvedType> {
        METHODS
            .iter()
            .filter(|entry| entry.0 == method)
            .find_map(|&(_, library, collection, index)| {
                state
                    .signatures
                    .instantiated_supertype(receiver, library, collection, &state.model)
                    .map(|instance| instance.type_argument(index))
            })
            .flatten()
    }

    fn check_call(
        &mut self,
        receiver: &ResolvedType,
        method: &str,
        args: ArgList<'a>,
        state: &SemanticState<'a>,
    ) {
        let mut positional = args.positional();
        if let Some(argument) = positional.next()
            && positional.next().is_none()
            && args.named().is_empty()
            && let Some(expected) = Self::expected_argument(receiver, method, state)
        {
            let context = Self::argument_context(receiver, method, state);
            self.check(argument, &expected, context.as_ref(), state);
        }
    }

    fn argument_context(
        receiver: &ResolvedType,
        method: &str,
        state: &SemanticState<'_>,
    ) -> Option<ResolvedType> {
        let (signature, substitutions) =
            state
                .signatures
                .resolved_member(receiver, method, &state.model)?;
        signature
            .positional
            .first()
            .map(|parameter| state.model.substitute(parameter, &substitutions))
    }

    fn check_index(&mut self, receiver: &ResolvedType, index: Expr<'a>, state: &SemanticState<'a>) {
        if let Some(map) =
            state
                .signatures
                .instantiated_supertype(receiver, "dart:core", "Map", &state.model)
            && let Some(expected) = map.type_argument(0)
        {
            let context = Self::argument_context(receiver, "[]", state);
            self.check(index, &expected, context.as_ref(), state);
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => match call.callee().unparenthesized() {
                Expr::Field(field) => {
                    self.check_call(
                        &receiver_type(field.object(), state),
                        field.field().name(),
                        call.args(),
                        state,
                    );
                }
                // The linter finds an implicit receiver only in classes,
                // mixins, enums, and extensions, not extension types.
                Expr::Ident(identifier) => {
                    if let Some(enclosing) = state.enclosing_type.as_ref()
                        && let ResolvedTypeView::Interface { identity, .. } = enclosing.view()
                        && state.signatures.type_kind(identity) != Some(TypeKind::ExtensionType)
                    {
                        self.check_call(enclosing, identifier.name(), call.args(), state);
                    }
                }
                _ => {}
            },
            Expr::Index(index) => {
                self.check_index(&receiver_type(index.object(), state), index.index(), state);
            }
            // Only a section's first selector has the cascade target as receiver.
            Expr::Cascade(cascade) => {
                let receiver = state.infer(cascade.object());
                for section in cascade.sections() {
                    match section.ops().next() {
                        Some(CascadeOp::Call(call)) => {
                            self.check_call(&receiver, call.name().name(), call.args(), state);
                        }
                        Some(CascadeOp::Index(index)) => {
                            self.check_index(&receiver, index.index(), state);
                        }
                        Some(CascadeOp::IndexAssign(assign)) => {
                            self.check_index(&receiver, assign.target(), state);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// The static type of an explicit receiver: `super` has the enclosing type.
fn receiver_type<'a>(object: Expr<'a>, state: &SemanticState<'a>) -> ResolvedType {
    match object.unparenthesized() {
        Expr::Super(_) => state
            .enclosing_type
            .clone()
            .unwrap_or(ResolvedType::UNKNOWN),
        other => state.infer(other),
    }
}
