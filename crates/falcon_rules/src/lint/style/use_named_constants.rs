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
    AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, Rule, RulePreparation, evaluate_constant,
    ConstantContext, evaluate_construction,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::ConstContexts;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseNamedConstants;

impl Rule for UseNamedConstants {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            contexts: ctx.const_contexts(program),
            own_field_initializers: own_field_initializers(program),
            local_classes: local_classes(program),
        }))
    }
}

/// Field-initializer ranges of each class declared in this file.
fn own_field_initializers(program: Program<'_>) -> Vec<(&str, Span)> {
    let mut ranges = Vec::new();
    for declaration in program.declarations() {
        let (name, members) = match declaration {
            TopLevelDecl::Class(class) => (class.name().name(), class.members()),
            TopLevelDecl::ExtensionType(extension) => {
                (extension.name().name(), extension.members())
            }
            _ => continue,
        };
        for member in members {
            if let ClassMember::Field(field) = member {
                ranges.extend(
                    field
                        .declarators()
                        .filter_map(VarDeclarator::initializer)
                        .map(|initializer| (name, initializer.span())),
                );
            }
        }
    }
    ranges
}

fn local_classes(program: Program<'_>) -> Vec<&str> {
    program
        .declarations()
        .filter_map(|declaration| match declaration {
            TopLevelDecl::Class(class) => Some(class.name().name()),
            TopLevelDecl::ExtensionType(extension) => Some(extension.name().name()),
            _ => None,
        })
        .collect()
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    contexts: &'a ConstContexts,
    own_field_initializers: Vec<(&'a str, Span)>,
    local_classes: Vec<&'a str>,
}

impl<'a> Collector<'a> {
    fn constructed_class(
        &self,
        node: Expr<'a>,
        state: &SemanticState<'a>,
    ) -> Option<DeclarationIdentity> {
        if !matches!(node.unparenthesized(), Expr::New(new) if new.is_const())
            && !self.contexts.contains_including_explicit(&node.span())
        {
            return None;
        }
        if let Expr::Call(call) = node.unparenthesized()
            && let Expr::DotShorthand(dot) = call.callee().unparenthesized()
        {
            if state.expected_unknown() {
                return None;
            }
            let Some(ResolvedTypeView::Interface { identity, .. }) = state.expected().map(ResolvedType::view) else {
                return None;
            };
            state.signatures.constructor(identity, dot.name().name())?;
            return Some(identity.clone());
        }
        crate::lint::flutter::constructor_invocation(node, &state.model)
            .map(|(identity, _, _)| identity)
    }

    fn matching_constant(
        &self,
        node: Expr<'a>,
        identity: &DeclarationIdentity,
        state: &SemanticState<'a>,
    ) -> Option<String> {
        let facts = state.signatures.declaration(identity)?;
        if facts.static_consts.is_empty() {
            return None;
        }
        let value = if let Expr::Call(call) = node.unparenthesized()
            && let Expr::DotShorthand(dot) = call.callee().unparenthesized()
            && let Some(ResolvedTypeView::Interface {
                arguments: type_args,
                ..
            }) = state.expected().map(ResolvedType::view)
        {
            evaluate_construction(identity, dot.name().name(), type_args, call.args(), ConstantContext::new(&state.model, &state.signatures))
            .value
        } else {
            let (_, constructor, args) =
                crate::lint::flutter::constructor_invocation(node, &state.model)?;
            // Context participates in omitted constructor type arguments.
            // Reinferring from values alone would equate, for example,
            // Single<Object>(true) with the distinct Single<bool>(true).
            let inferred = state.infer_with_context(node, state.expected());
            let arguments: Vec<_> = inferred.type_arguments().collect();
            evaluate_construction(identity, constructor, &arguments, args, ConstantContext::new(&state.model, &state.signatures))
            .value
        }?;
        let fields: HashMap<&str, Expr<'_>> = facts
            .static_consts
            .iter()
            .map(|field| (field.name.as_str(), field.initializer.expr()))
            .collect();
        let declared_here = self
            .local_classes
            .iter()
            .any(|name| *name == identity.name());
        facts
            .static_consts
            .iter()
            .filter(|field| declared_here || !field.name.starts_with('_'))
            .find(|field| {
                evaluate_constant(field.initializer.expr(), ConstantContext { owner: Some(identity), fields: Some(&fields), ..ConstantContext::new(&state.model, &state.signatures) })
                .as_ref()
                    == Some(&value)
            })
            .map(|field| format!("{}.{}", identity.name(), field.name))
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(node, Expr::Paren(_)) {
            return;
        }
        let Some(identity) = self.constructed_class(node, state) else {
            return;
        };
        let span = node.span();
        let in_own_field = self.own_field_initializers.iter().any(|(class, range)| {
            *class == identity.name() && range.contains(span)
        });
        if in_own_field {
            return;
        }
        if let Some(constant) = self.matching_constant(node, &identity, state) {
            self.diags.push(self.ctx.diagnostic(format!(
                "Use the constant `{constant}` rather than a constructor returning the same object."
            ), span));
        }
    }
}
