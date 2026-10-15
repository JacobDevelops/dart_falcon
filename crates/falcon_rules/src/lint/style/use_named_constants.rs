//! Prefers a class's predefined static constant to constructing an equal constant.
//!
//! When a class declares `static const zero = A(0);`, writing `const A(0)`
//! elsewhere creates the very same canonicalized object; referring to `A.zero`
//! names the intent. The rule fires on a constant constructor invocation (an
//! explicit `const` creation or a constructor call in a constant context) whose
//! value equals an accessible `static const` field of the constructed class. An
//! invocation inside a field initializer of that same class is exempt, so
//! `static const a = A(); static const b = A();` is fine.
//!
//! falcon compares canonical constant values with its conservative evaluator.
//! Named and redirecting constructors are compared by their resulting instance,
//! not their constructor spelling. Unavailable declarations and values the
//! evaluator cannot fold are not checked.
//!
//! Adopted from the official `use_named_constants` lint.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, evaluate_constant,
    evaluate_construction,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::const_eval::ConstructorConstContexts as ConstContexts;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UseNamedConstants;

impl Rule for UseNamedConstants {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            contexts: ConstContexts::new(program),
            own_field_initializers: own_field_initializers(program),
            local_classes: local_classes(program),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

/// Field-initializer ranges of each class declared in this file.
fn own_field_initializers(program: &Program) -> Vec<(String, Span)> {
    let mut ranges = Vec::new();
    for declaration in &program.declarations {
        let (name, members) = match declaration {
            TopLevelDecl::Class(class) => (&class.name.name, &class.members),
            TopLevelDecl::ExtensionType(extension) => (&extension.name.name, &extension.members),
            _ => continue,
        };
        for member in members {
            if let ClassMember::Field(field) = member {
                ranges.extend(
                    field
                        .declarators
                        .iter()
                        .filter_map(|declarator| declarator.initializer.as_ref())
                        .map(|initializer| (name.clone(), *initializer.span())),
                );
            }
        }
    }
    ranges
}

fn local_classes(program: &Program) -> Vec<String> {
    program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            TopLevelDecl::Class(class) => Some(class.name.name.clone()),
            TopLevelDecl::ExtensionType(extension) => Some(extension.name.name.clone()),
            _ => None,
        })
        .collect()
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    contexts: ConstContexts,
    own_field_initializers: Vec<(String, Span)>,
    local_classes: Vec<String>,
}

impl Collector<'_> {
    fn constructed_class(
        &self,
        node: &Expr,
        state: &SemanticState<'_>,
    ) -> Option<DeclarationIdentity> {
        if !matches!(node.unparenthesized(), Expr::New { is_const: true, .. })
            && !self.contexts.contains(node.span())
        {
            return None;
        }
        if let Expr::Call { callee, .. } = node.unparenthesized()
            && let Expr::DotShorthand { name, .. } = callee.unparenthesized()
        {
            if state.expected_unknown() {
                return None;
            }
            let Some(ResolvedType::Interface { identity, .. }) = state.expected() else {
                return None;
            };
            state.signatures.constructor(identity, &name.name)?;
            return Some(identity.clone());
        }
        crate::lint::flutter::constructor_invocation(node, &state.model)
            .map(|(identity, _, _)| identity)
    }

    fn matching_constant(
        &self,
        node: &Expr,
        identity: &DeclarationIdentity,
        state: &SemanticState<'_>,
    ) -> Option<String> {
        let facts = state.signatures.declaration(identity)?;
        if facts.static_consts.is_empty() {
            return None;
        }
        let empty = HashMap::new();
        let value = if let Expr::Call { callee, args, .. } = node.unparenthesized()
            && let Expr::DotShorthand { name, .. } = callee.unparenthesized()
            && let Some(ResolvedType::Interface {
                arguments: type_args,
                ..
            }) = state.expected()
        {
            evaluate_construction(
                identity,
                &name.name,
                type_args,
                args,
                &empty,
                &state.model,
                &state.signatures,
            )
            .value
        } else {
            let (_, constructor, args) =
                crate::lint::flutter::constructor_invocation(node, &state.model)?;
            // Context participates in omitted constructor type arguments.
            // Reinferring from values alone would equate, for example,
            // Single<Object>(true) with the distinct Single<bool>(true).
            let inferred = state.infer_with_context(node, state.expected());
            let arguments: Vec<_> = inferred.type_arguments().collect();
            evaluate_construction(
                identity,
                constructor,
                &arguments,
                args,
                &empty,
                &state.model,
                &state.signatures,
            )
            .value
        }?;
        let fields: HashMap<&str, &Expr> = facts
            .static_consts
            .iter()
            .map(|field| (field.name.as_str(), &field.initializer))
            .collect();
        let declared_here = self
            .local_classes
            .iter()
            .any(|name| name == identity.name());
        facts
            .static_consts
            .iter()
            .filter(|field| declared_here || !field.name.starts_with('_'))
            .find(|field| {
                evaluate_constant(
                    &field.initializer,
                    Some(identity),
                    &fields,
                    &state.model,
                    &state.signatures,
                )
                .as_ref()
                    == Some(&value)
            })
            .map(|field| format!("{}.{}", identity.name(), field.name))
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if matches!(node, Expr::Paren { .. }) {
            return;
        }
        let Some(identity) = self.constructed_class(node, state) else {
            return;
        };
        let span = node.span();
        let in_own_field = self.own_field_initializers.iter().any(|(class, range)| {
            class == identity.name() && range.start <= span.start && span.end <= range.end
        });
        if in_own_field {
            return;
        }
        if let Some(constant) = self.matching_constant(node, &identity, state) {
            self.diags.push(self.ctx.diagnostic(
                "use-named-constants",
                format!(
                    "Use the constant '{constant}' rather than a constructor returning the same object."
                ),
                *span,
            ));
        }
    }
}
