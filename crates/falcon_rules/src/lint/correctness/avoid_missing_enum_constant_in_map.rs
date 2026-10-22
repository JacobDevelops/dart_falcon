//! Require enum-keyed map literals to include every declared enum value.
//!
//! Key types and constant identities come from resolution, including contextual
//! types, imported enums, aliases and dot shorthands. Build-independent constant
//! branches and statically known spreads contribute keys; unknown coverage disables the check.

use crate::lint::support::constants::constant_value;
use std::collections::HashSet;

use falcon_analyze::{ConstantContext, evaluate_constant};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, ResolvedType, Rule, RulePreparation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMissingEnumConstantInMap;

impl Rule for AvoidMissingEnumConstantInMap {
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

fn constant(
    expression: Expr<'_>,
    expected: Option<&ResolvedType>,
    state: &SemanticState<'_>,
) -> Option<ConstantValue> {
    // Captured local values discard build provenance; evaluate their source instead.
    let expression = if let Expr::Ident(identifier) = expression.unparenthesized()
        && let Some(initializer) = state.local_constant(identifier.name())
    {
        // Local reads may have been shadowed since declaration; don't rebind them.
        let mut local_reference = false;
        falcon_syntax::cst::visit::for_each_expr(initializer, &mut |node| {
            local_reference |= matches!(node, Expr::Ident(name)
                if state.environment.is_bound(name.name()));
        });
        if local_reference {
            return None;
        }
        initializer
    } else {
        expression
    };
    evaluate_constant(expression, ConstantContext { owner: state.constant_owner.as_ref(), locals: Some(&state.visible_local_constants()), expected, stable: true, ..ConstantContext::new(&state.model, &state.signatures) })
}

fn cover_key(
    value: ConstantValue,
    owner: &DeclarationIdentity,
    covered: &mut HashSet<String>,
) -> bool {
    match value {
        ConstantValue::EnumValue(identity, name) if &identity == owner => {
            covered.insert(name);
            true
        }
        ConstantValue::Null => true,
        _ => false,
    }
}

// A coverage proof, not a traversal: follow only an evaluated constant branch,
// recurse into literal spreads, and reject any element with unknown coverage.
fn cover_element(
    element: MapElement<'_>,
    key_type: &ResolvedType,
    owner: &DeclarationIdentity,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
    covered: &mut HashSet<String>,
) -> bool {
    match element {
        MapElement::Entry(entry) => {
            // Null-aware values can omit their key even when the key is known.
            if entry.value_null_aware() {
                return false;
            }
            constant(entry.key(), Some(key_type), state)
                .is_some_and(|value| cover_key(value, owner, covered))
        }
        MapElement::Spread(spread) => {
            if let Expr::Map(map) = spread.expr().unparenthesized() {
                return map
                    .elements()
                    .all(|element| cover_element(element, key_type, owner, state, ctx, covered));
            }
            match constant(spread.expr(), None, state) {
                Some(ConstantValue::Map(_, entries)) => entries
                    .into_iter()
                    .all(|(key, _)| cover_key(key, owner, covered)),
                Some(ConstantValue::Null) => spread.is_null_aware(),
                None
                | Some(
                    ConstantValue::Bool(_)
                    | ConstantValue::Number(_)
                    | ConstantValue::String(_)
                    | ConstantValue::List(..)
                    | ConstantValue::Set(..)
                    | ConstantValue::Record(_)
                    | ConstantValue::Instance(..)
                    | ConstantValue::EnumValue(..)
                    | ConstantValue::Type(_)
                    | ConstantValue::TearOff(..),
                ) => false,
            }
        }
        MapElement::If(branch) => {
            let IfCondition::Expr(condition) = branch.condition() else {
                return false;
            };
            match constant_value(condition, state, ctx) {
                Some(ConstantValue::Bool(true)) => {
                    cover_element(branch.then_entry(), key_type, owner, state, ctx, covered)
                }
                Some(ConstantValue::Bool(false)) => branch.else_entry().is_none_or(|element| {
                    cover_element(element, key_type, owner, state, ctx, covered)
                }),
                None
                | Some(
                    ConstantValue::Null
                    | ConstantValue::Number(_)
                    | ConstantValue::String(_)
                    | ConstantValue::List(..)
                    | ConstantValue::Set(..)
                    | ConstantValue::Map(..)
                    | ConstantValue::Record(_)
                    | ConstantValue::Instance(..)
                    | ConstantValue::EnumValue(..)
                    | ConstantValue::Type(_)
                    | ConstantValue::TearOff(..),
                ) => false,
            }
        }
        MapElement::For(_) => false,
    }
}

fn spread_fragment(map: MapExpr<'_>) -> bool {
    let mut node = map.syntax();
    while let Some(parent) = node.parent() {
        if matches!(MapElement::cast(parent), Some(MapElement::Spread(_))) {
            return true;
        }
        match Expr::cast(parent) {
            Some(Expr::Paren(_) | Expr::Conditional(_)) => node = parent,
            _ => return false,
        }
    }
    false
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Map(map) = node else {
            return;
        };
        if spread_fragment(map) {
            return;
        }
        let ty = state.infer_with_context(node, state.expected());
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = &ty
        else {
            return;
        };
        if !identity.is_sdk("dart:core", "Map") {
            return;
        }
        let Some(key_type @ ResolvedType::Interface { identity, .. }) = arguments.first() else {
            return;
        };
        let Some(values) = state.signatures.enum_value_names(identity) else {
            return;
        };
        let mut covered = HashSet::new();
        if !map
            .elements()
            .all(|element| cover_element(element, key_type, identity, state, self.ctx, &mut covered))
        {
            return;
        }
        // Empty maps are commonly mutable caches or accumulators, not enum tables.
        if covered.is_empty() {
            return;
        }
        let missing: Vec<_> = values
            .iter()
            .filter(|value| !covered.contains(*value))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            self.diagnostics.push(self.ctx.diagnostic(format!(
                "Include every `{}` value in this map; missing: `{}`.",
                identity.name(),
                missing.join("`, `")
            ), map.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
