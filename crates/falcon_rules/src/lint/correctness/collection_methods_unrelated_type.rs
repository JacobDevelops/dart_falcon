//! Reject collection operations whose argument is proven unrelated to K, V, or E.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth};
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

impl Collector<'_> {
    fn check(&mut self, argument: &Expr, expected: &ResolvedType, state: &SemanticState<'_>) {
        let actual = state.infer(argument);
        if state.signatures.unrelated(&actual, expected, &state.model) != Truth::Yes {
            return;
        }
        let span = argument.span();
        self.diagnostics.push(self.ctx.diagnostic(
            "collection-methods-unrelated-type",
            "The argument type is unrelated to this collection's element type.",
            *span,
        ));
    }

    fn expected_argument(
        &self,
        receiver: &ResolvedType,
        method: &str,
        state: &SemanticState<'_>,
    ) -> Option<ResolvedType> {
        if matches!(
            receiver,
            ResolvedType::Interface {
                identity: DeclarationIdentity::Project { .. },
                ..
            }
        ) && state
            .signatures
            .resolved_member(receiver, method, &state.model)
            .is_some()
        {
            return None;
        }
        if let Some(map) =
            state
                .signatures
                .instantiated_supertype(receiver, "dart:core", "Map", &state.model)
        {
            return match method {
                "containsKey" | "remove" => map.arguments().first().cloned(),
                "containsValue" => map.arguments().get(1).cloned(),
                _ => None,
            };
        }
        if matches!(method, "lookup" | "remove")
            && let Some(set) =
                state
                    .signatures
                    .instantiated_supertype(receiver, "dart:core", "Set", &state.model)
        {
            return set.arguments().first().cloned();
        }
        if method == "remove"
            && let Some(list) =
                state
                    .signatures
                    .instantiated_supertype(receiver, "dart:core", "List", &state.model)
        {
            return list.arguments().first().cloned();
        }
        if method == "remove"
            && let Some(queue) = state.signatures.instantiated_supertype(
                receiver,
                "dart:collection",
                "Queue",
                &state.model,
            )
        {
            return queue.arguments().first().cloned();
        }
        if method == "contains"
            && let Some(iterable) = state.signatures.instantiated_supertype(
                receiver,
                "dart:core",
                "Iterable",
                &state.model,
            )
        {
            return iterable.arguments().first().cloned();
        }
        None
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Call { callee, args, .. } => {
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    let receiver = state.infer(object);
                    if let Some(expected) = self.expected_argument(&receiver, &field.name, state)
                        && let Some(argument) = args.positional.first()
                    {
                        self.check(argument, &expected, state);
                    }
                }
            }
            Expr::Index { object, index, .. } => {
                let receiver = state.infer(object);
                if let Some(map) = state.signatures.instantiated_supertype(
                    &receiver,
                    "dart:core",
                    "Map",
                    &state.model,
                ) && let Some(expected) = map.arguments().first()
                {
                    self.check(index, expected, state);
                }
            }
            _ => {}
        }
    }
}
