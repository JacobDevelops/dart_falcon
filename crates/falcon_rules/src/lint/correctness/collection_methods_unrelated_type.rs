//! Reports collection lookups whose argument type is unrelated to the
//! collection's type argument.
//!
//! Adopted from the official `collection_methods_unrelated_type` lint. With
//! exactly one argument, `Iterable<E>.contains`, `List<E>.remove`,
//! `Map<K, V>.containsKey`/`containsValue`/`remove`, `Queue<E>.remove`,
//! `Set<E>.lookup`/`remove`, and `Map<K, V>.[]` are checked on a receiver
//! (explicit, or the implicit `this` of the enclosing type) that implements the
//! declaring collection, whatever the receiver overrides. The argument must not
//! be unrelated ([`SignatureIndex::types_are_unrelated`]) to the type argument.
//!
//! [`SignatureIndex::types_are_unrelated`]: falcon_analyze::SignatureIndex::types_are_unrelated

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct CollectionMethodsUnrelatedType;

impl Rule for CollectionMethodsUnrelatedType {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
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

impl Collector<'_> {
    fn check(
        &mut self,
        argument: &Expr,
        expected: &ResolvedType,
        context: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        let actual = state.infer_with_context(argument, context);
        if state
            .signatures
            .types_are_unrelated(&actual, expected, &state.model)
            != Truth::Yes
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "collection-methods-unrelated-type",
            "The argument type is unrelated to this collection's element type.",
            *argument.span(),
        ));
    }

    fn expected_argument(
        &self,
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
        args: &ArgList,
        state: &SemanticState<'_>,
    ) {
        if let [argument] = args.positional.as_slice()
            && args.named.is_empty()
            && let Some(expected) = self.expected_argument(receiver, method, state)
        {
            let context = self.argument_context(receiver, method, state);
            self.check(argument, &expected, context.as_ref(), state);
        }
    }

    fn argument_context(
        &self,
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

    fn check_index(&mut self, receiver: &ResolvedType, index: &Expr, state: &SemanticState<'_>) {
        if let Some(map) =
            state
                .signatures
                .instantiated_supertype(receiver, "dart:core", "Map", &state.model)
            && let Some(expected) = map.type_argument(0)
        {
            let context = self.argument_context(receiver, "[]", state);
            self.check(index, &expected, context.as_ref(), state);
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Call { callee, args, .. } => match callee.unparenthesized() {
                Expr::Field { object, field, .. } => {
                    self.check_call(&receiver_type(object, state), &field.name, args, state);
                }
                // The linter finds an implicit receiver only in classes,
                // mixins, enums, and extensions, not extension types.
                Expr::Ident(identifier) => {
                    if let Some(enclosing @ ResolvedType::Interface { identity, .. }) =
                        &state.enclosing_type
                        && state.signatures.type_kind(identity) != Some(TypeKind::ExtensionType)
                    {
                        self.check_call(enclosing, &identifier.name, args, state);
                    }
                }
                _ => {}
            },
            Expr::Index { object, index, .. } => {
                self.check_index(&receiver_type(object, state), index, state);
            }
            // Only a section's first selector has the cascade target as receiver.
            Expr::Cascade {
                object, sections, ..
            } => {
                let receiver = state.infer(object);
                for section in sections {
                    match section.ops.first() {
                        Some(CascadeOp::Call(method, _, args)) => {
                            self.check_call(&receiver, &method.name, args, state);
                        }
                        Some(
                            CascadeOp::Index { index, .. } | CascadeOp::IndexAssign(index, _, _),
                        ) => {
                            self.check_index(&receiver, index, state);
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
fn receiver_type(object: &Expr, state: &SemanticState<'_>) -> ResolvedType {
    match object.unparenthesized() {
        Expr::Super { .. } => state
            .enclosing_type
            .clone()
            .unwrap_or(ResolvedType::Unknown),
        other => state.infer(other),
    }
}
