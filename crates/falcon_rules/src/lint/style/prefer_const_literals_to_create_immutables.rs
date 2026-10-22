//! Prefers `const` list, set, and map literals as arguments to constructors of `@immutable` classes.
//!
//! An instance of an `@immutable` class should not hold a mutable collection.
//! A non-const collection literal passed
//! directly to such a constructor — or nested only inside other collection
//! literals in that argument — is flagged when it could be `const`: every
//! element is a provable constant and set elements and map keys have primitive
//! equality. A class is immutable when it or a class on its `extends` chain is
//! annotated `@immutable` or `@Immutable(...)` from `package:meta`.
//! Literals already in a constant context are implicitly const and are left
//! alone, as are arguments to classes whose immutability cannot be established.
//! Diagnostic ranges exclude parentheses surrounding the collection literal.
//!
//! ```dart
//! @immutable
//! class A {
//!   A(this.values);
//!   final List<int> values;
//! }
//!
//! final a = A([1]);       // BAD
//! final b = A(const [1]); // GOOD
//! ```
//!
//! Adopted from the official `prefer_const_literals_to_create_immutables` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::const_eval::{ConstContexts, constant_in_context, deferred_prefixes};
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferConstLiteralsToCreateImmutables;

impl Rule for PreferConstLiteralsToCreateImmutables {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            contexts: ConstContexts::of(program),
            deferred: deferred_prefixes(program),
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    contexts: ConstContexts,
    deferred: Vec<&'a str>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(node, Expr::New(new) if new.is_const()) || self.contexts.contains(&node.span())
        {
            return;
        }
        let Some((identity, _, args)) =
            crate::lint::flutter::constructor_invocation(node, &state.model)
        else {
            return;
        };
        if !is_immutable(&identity, state) {
            return;
        }
        let arguments = args
            .positional()
            .chain(args.named().map(|argument| argument.value()));
        for literal in arguments.flat_map(collection_literals) {
            if constant_in_context(literal, &self.deferred, state) {
                self.diags.push(self.ctx.diagnostic("Use `const` literals as arguments to constructors of `@immutable` classes.", literal.unparenthesized().span()));
            }
        }
    }
}

/// Whether the class or a class on its superclass chain (`extends` only, not
/// mixins or interfaces) has meta's `@immutable` or `@Immutable(...)` annotation.
fn is_immutable(identity: &DeclarationIdentity, state: &SemanticState<'_>) -> bool {
    let signatures = &state.signatures;
    let annotated = |identity: &DeclarationIdentity| {
        signatures.declaration(identity).is_some_and(|facts| {
            facts
                .annotations
                .iter()
                .any(DeclarationIdentity::is_meta_immutable)
        })
    };
    // A cyclic (erroneous) hierarchy ends the walk after a bounded depth.
    let mut current = identity;
    for _ in 0..64 {
        if annotated(current) {
            return true;
        }
        match signatures.superclass(current) {
            Some(ResolvedType::Interface { identity, .. }) => current = identity,
            Some(_) | None => break,
        }
    }
    false
}

/// Non-const collection literals reachable from an argument through
/// collection-literal elements and map entries only, outermost first.
fn collection_literals(argument: Expr<'_>) -> Vec<Expr<'_>> {
    let mut literals = Vec::new();
    if !matches!(
        argument.unparenthesized(),
        Expr::List(_) | Expr::Set(_) | Expr::Map(_)
    ) {
        return literals;
    }
    let mut pending = vec![argument];
    while let Some(expression) = pending.pop() {
        match expression.unparenthesized() {
            Expr::List(list) if !list.is_const() => {
                literals.push(expression);
                pending.extend(list.elements().filter_map(|element| match element {
                    CollectionElement::Expr(element) => Some(element),
                    _ => None,
                }));
            }
            Expr::Set(set) if !set.is_const() => {
                literals.push(expression);
                pending.extend(set.elements().filter_map(|element| match element {
                    CollectionElement::Expr(element) => Some(element),
                    _ => None,
                }));
            }
            Expr::Map(map) if !map.is_const() => {
                literals.push(expression);
                for element in map.elements() {
                    if let MapElement::Entry(entry) = element {
                        pending.push(entry.key());
                        pending.push(entry.value());
                    }
                }
            }
            _ => {}
        }
    }
    literals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn collection_ranges_exclude_surrounding_parentheses() {
        let source = "import 'package:meta/meta.dart'; @immutable class Config { final List<Object> values; Config(this.values); } void f() { Config(([1])); Config(([[2]])); }";
        let diagnostics = inference::analyze(PreferConstLiteralsToCreateImmutables::NAME, &PreferConstLiteralsToCreateImmutables, Path::new("literal_spans.dart"),
        source,
        &FalconConfig::default(),);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["[1]", "[[2]]", "[2]"]);
    }
}
