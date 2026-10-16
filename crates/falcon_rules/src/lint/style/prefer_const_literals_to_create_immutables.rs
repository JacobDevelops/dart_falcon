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

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::const_eval::{ConstContexts, constant_in_context, deferred_prefixes};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct PreferConstLiteralsToCreateImmutables;

impl Rule for PreferConstLiteralsToCreateImmutables {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            contexts: ConstContexts::of(program),
            deferred: deferred_prefixes(program),
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    contexts: ConstContexts,
    deferred: Vec<&'a str>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if matches!(node, Expr::New { is_const: true, .. }) || self.contexts.contains(node.span()) {
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
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value));
        for literal in arguments.flat_map(collection_literals) {
            if constant_in_context(literal, &self.deferred, state) {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-const-literals-to-create-immutables",
                    "Use 'const' literals as arguments to constructors of '@immutable' classes.",
                    *literal.unparenthesized().span(),
                ));
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
fn collection_literals(argument: &Expr) -> Vec<&Expr> {
    let mut literals = Vec::new();
    if !matches!(
        argument.unparenthesized(),
        Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. }
    ) {
        return literals;
    }
    let mut pending = vec![argument];
    while let Some(expression) = pending.pop() {
        match expression.unparenthesized() {
            Expr::List {
                is_const: false,
                elements,
                ..
            }
            | Expr::Set {
                is_const: false,
                elements,
                ..
            } => {
                literals.push(expression);
                pending.extend(elements.iter().filter_map(|element| match element {
                    CollectionElement::Expr(element) => Some(element),
                    CollectionElement::NullAware { .. }
                    | CollectionElement::Spread { .. }
                    | CollectionElement::If { .. }
                    | CollectionElement::For { .. } => None,
                }));
            }
            Expr::Map {
                is_const: false,
                elements,
                ..
            } => {
                literals.push(expression);
                for element in elements {
                    if let MapElement::Entry(entry) = element {
                        pending.push(&entry.key);
                        pending.push(&entry.value);
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
        let diagnostics = inference::analyze(
            &PreferConstLiteralsToCreateImmutables,
            Path::new("literal_spans.dart"),
            source,
            &FalconConfig::default(),
        );
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["[1]", "[[2]]", "[2]"]);
    }
}
