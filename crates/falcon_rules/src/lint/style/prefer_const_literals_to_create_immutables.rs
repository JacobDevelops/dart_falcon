//! Prefers `const` list, set, and map literals as arguments to constructors of `@immutable` classes.
//!
//! An instance of an `@immutable` class (every Flutter `Widget` is one) should
//! not hold a mutable collection. A non-const collection literal passed
//! directly to such a constructor — or nested only inside other collection
//! literals in that argument — is flagged when it could be `const`: every
//! element is a provable constant. Literals already in a constant context are
//! implicitly const and are left alone, as are arguments to classes whose
//! immutability cannot be established.
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

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::const_eval::{ConstContexts, constant_in_context};
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
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    contexts: ConstContexts,
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
        if !is_immutable(&identity, state, self.ctx) {
            return;
        }
        let arguments = args
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value));
        for literal in arguments.flat_map(collection_literals) {
            if constant_in_context(literal, state) {
                self.diags.push(self.ctx.diagnostic(
                    "prefer-const-literals-to-create-immutables",
                    "Use 'const' literals as arguments to constructors of '@immutable' classes.",
                    *literal.span(),
                ));
            }
        }
    }
}

/// Whether the class or a class it extends is annotated `@immutable` (as
/// Flutter's `Widget` is). Mixins and interfaces do not count.
fn is_immutable(
    identity: &DeclarationIdentity,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext<'_>,
) -> bool {
    let annotated = state.signatures.declaration(identity).is_some_and(|facts| {
        facts
            .annotations
            .iter()
            .any(|annotation| annotation.is_package("meta", "immutable"))
    });
    let declared_immutable = || {
        matches!(identity, DeclarationIdentity::Project { name, .. }
            if ctx.types.is_some_and(|types| types.superclass_chain_is_immutable(name) == Truth::Yes))
    };
    let widget = || {
        state.signatures.is_subtype_of(
            &ResolvedType::interface_identity(identity.clone()),
            &DeclarationIdentity::flutter("Widget"),
            &state.model,
        ) == Truth::Yes
    };
    annotated || declared_immutable() || widget()
}

/// Non-const collection literals reachable from an argument through
/// collection-literal elements and map entries only, outermost first.
fn collection_literals(argument: &Expr) -> Vec<&Expr> {
    let mut literals = Vec::new();
    if !matches!(
        argument,
        Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. }
    ) {
        return literals;
    }
    let mut pending = vec![argument];
    while let Some(expression) = pending.pop() {
        match expression {
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
