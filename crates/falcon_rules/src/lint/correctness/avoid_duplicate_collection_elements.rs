//! Flags repeated scalar literals and stable local reads in list and set literals.
//!
//! Duplicate entries often indicate a copy-and-paste mistake. Comparisons retain
//! literal spelling. Calls, getters, spreads, and control-flow elements end the
//! comparison region, since they can change values or contribute unknown items.
//! Map keys are covered separately by `avoid-duplicate-map-keys`.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use super::avoid_duplicate_map_keys::scalar_literal;
use crate::lint::expressions::equivalent;

#[derive(Default)]
pub struct AvoidDuplicateCollectionElements;

impl Rule for AvoidDuplicateCollectionElements {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        match expression {
            Expr::List(list) => self.check(list.elements(), state),
            Expr::Set(set) => self.check(set.elements(), state),
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'a> Collector<'a> {
    fn check(
        &mut self,
        elements: impl Iterator<Item = CollectionElement<'a>>,
        state: &SemanticState<'a>,
    ) {
        let mut seen = Vec::new();
        for element in elements {
            let CollectionElement::Expr(expression) = element else {
                seen.clear();
                continue;
            };
            let expression = expression.unparenthesized();
            let stable_local = matches!(expression, Expr::Ident(identifier)
                if state.is_variable_read(identifier.name())
                    && state.environment.binding_declaration(identifier.name()).is_some()
                    && !state.is_own_field(identifier.name()))
                && matches!(state.infer(expression), ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, .. }
                    if library.as_str() == "dart:core" && matches!(name.as_str(), "bool" | "int" | "double" | "num" | "String"));
            if !scalar_literal(expression) && !stable_local {
                seen.clear();
                continue;
            }
            if seen
                .iter()
                .any(|previous| equivalent(*previous, expression, self.ctx))
            {
                self.diagnostics.push(self.ctx.diagnostic(
                    "avoid-duplicate-collection-elements",
                    "Duplicate collection element. Remove or replace the repeated element.",
                    expression.span(),
                ));
            } else {
                seen.push(expression);
            }
        }
    }
}
