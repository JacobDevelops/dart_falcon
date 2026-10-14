//! Disallow unused Flutter framework and `dart:async` imports.
//!
//! Usage is based on resolved symbol identity and import combinators. Unknown
//! unprefixed names conservatively keep an unrestricted import, avoiding guesses
//! from capitalization or substrings.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_annotation, walk_dart_type};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnnecessaryFlutterImports;

impl Rule for UnnecessaryFlutterImports {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut usage = Usage::default();
        visit_program(&mut usage, program, state);

        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let model = falcon_analyze::SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut type_usage = TypeUsage {
            model: &model,
            unprefixed: &mut usage.unprefixed,
            prefixed: &mut usage.prefixed,
            unknown: &mut usage.unknown,
        };
        type_usage.visit_program(program);

        program
            .imports
            .iter()
            .filter(|import| import_targets_framework(import))
            .filter(|import| !import_is_used(import, &usage))
            .map(|import| {
                ctx.diagnostic(
                    "unnecessary-flutter-imports",
                    "Unnecessary Flutter import. Remove unused imports.",
                    import.span,
                )
            })
            .collect()
    }
}

#[derive(Default)]
struct Usage {
    unprefixed: HashSet<String>,
    prefixed: HashSet<(String, String)>,
    unknown: HashSet<String>,
}

impl SemanticRuleVisitor for Usage {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Ident(identifier) if !state.environment.is_bound(&identifier.name) => {
                let names = &identifier.name;
                match state.model.resolve_imported_member(names) {
                    Some(identity) if framework_identity(&identity) => {
                        self.unprefixed.insert(identifier.name.clone());
                    }
                    None if state.model.resolve_name(names).is_none()
                        && state.model.resolve_value(names).is_none()
                        && state.model.resolve_sdk_member(names).is_none() =>
                    {
                        self.unknown.insert(identifier.name.clone());
                    }
                    _ => {}
                }
            }
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(prefix) = object.as_ref()
                    && !state.environment.is_bound(&prefix.name)
                {
                    let names = QualifiedName::prefixed(&prefix.name, &field.name);
                    match state.model.resolve_imported_member(names) {
                        Some(identity) if framework_identity(&identity) => {
                            self.prefixed
                                .insert((prefix.name.clone(), field.name.clone()));
                        }
                        None => {
                            self.unknown.insert(prefix.name.clone());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

struct TypeUsage<'a, 'model> {
    model: &'a falcon_analyze::SemanticModel<'model>,
    unprefixed: &'a mut HashSet<String>,
    prefixed: &'a mut HashSet<(String, String)>,
    unknown: &'a mut HashSet<String>,
}

impl Visitor for TypeUsage<'_, '_> {
    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Named(named) = node {
            let segments = named
                .segments
                .iter()
                .map(|segment| segment.name.clone())
                .collect::<Vec<_>>();
            match self.model.resolve_type(node) {
                ResolvedType::Interface { identity, .. } if framework_identity(&identity) => {
                    if let [name] = segments.as_slice() {
                        self.unprefixed.insert(name.clone());
                    } else if let [prefix, name] = segments.as_slice() {
                        self.prefixed.insert((prefix.clone(), name.clone()));
                    }
                }
                ResolvedType::Unknown => {
                    if let Some(first) = segments.first() {
                        self.unknown.insert(first.clone());
                    }
                }
                _ => {}
            }
        }
        walk_dart_type(self, node);
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        if let Some(name) = QualifiedName::from_identifiers(&node.name)
            && let Some(identity) = self.model.resolve_imported_member(name)
            && framework_identity(&identity)
        {
            match name.prefix {
                None => self.unprefixed.insert(name.name.to_string()),
                Some(prefix) => self
                    .prefixed
                    .insert((prefix.to_string(), name.name.to_string())),
            };
        }
        walk_annotation(self, node);
    }
}

fn framework_identity(identity: &DeclarationIdentity) -> bool {
    matches!(
        identity,
        DeclarationIdentity::Package {
            package: "flutter",
            ..
        } | DeclarationIdentity::Sdk {
            library: "dart:async",
            ..
        }
    )
}

fn import_targets_framework(import: &ImportDirective) -> bool {
    std::iter::once(&import.uri)
        .chain(import.configurable_uris.iter().map(|uri| &uri.uri))
        .any(|uri| uri.value.starts_with("package:flutter/") || uri.value == "dart:async")
}

fn import_is_used(import: &ImportDirective, usage: &Usage) -> bool {
    if let Some(prefix) = &import.as_name {
        return usage
            .prefixed
            .iter()
            .any(|(used_prefix, name)| used_prefix == &prefix.name && allows(import, name))
            || usage.unknown.contains(&prefix.name);
    }
    usage.unprefixed.iter().any(|name| allows(import, name))
        || usage.unknown.iter().any(|name| allows(import, name))
}

fn allows(import: &ImportDirective, name: &str) -> bool {
    import
        .combinators
        .iter()
        .all(|combinator| match combinator {
            ImportCombinator::Show(names, _) => names.iter().any(|shown| shown.name == name),
            ImportCombinator::Hide(names, _) => names.iter().all(|hidden| hidden.name != name),
        })
}
