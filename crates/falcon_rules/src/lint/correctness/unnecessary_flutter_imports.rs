//! Disallow unused Flutter framework and `dart:async` imports.
//!
//! Usage is based on resolved symbol identity and import combinators. Unknown
//! unprefixed names conservatively keep an unrestricted import, avoiding guesses
//! from capitalization or substrings.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedTypeView, Rule, RulePreparation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_annotation, walk_dart_type};
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UnnecessaryFlutterImports;

impl Rule for UnnecessaryFlutterImports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Usage {
            program,
            ctx,
            unprefixed: HashSet::new(),
            prefixed: HashSet::new(),
            unknown: HashSet::new(),
        }))
    }
}

struct Usage<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    unprefixed: HashSet<&'a str>,
    prefixed: HashSet<(&'a str, &'a str)>,
    unknown: HashSet<&'a str>,
}

impl<'a> SemanticRuleVisitor<'a> for Usage<'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        let program = self.program;
        let ctx = self.ctx;

        let Some(identities) = ctx.identities() else {
            return Vec::new();
        };
        let model = falcon_analyze::SemanticModel::new(
            ctx.file_path(),
            ctx.parsed(),
            identities,
            ctx.types(),
        );
        let usage = &mut *self;
        let mut type_usage = TypeUsage {
            model: &model,
            unprefixed: &mut usage.unprefixed,
            prefixed: &mut usage.prefixed,
            unknown: &mut usage.unknown,
        };
        type_usage.visit_program(program);

        program
            .imports()
            .filter(|import| import_targets_framework(*import))
            .filter(|import| !import_is_used(*import, &self))
            .map(|import| {
                ctx.diagnostic("Remove this unnecessary Flutter import.", import.span())
            })
            .collect()
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        match node {
            Expr::Ident(identifier) if !state.environment.is_bound(identifier.name()) => {
                let names = identifier.name();
                match state.model.resolve_imported_member(names) {
                    Some(identity) if framework_identity(&identity) => {
                        self.unprefixed.insert(identifier.name());
                    }
                    None if state.model.resolve_name(names).is_none()
                        && state.model.resolve_value(names).is_none()
                        && state.model.resolve_sdk_member(names).is_none() =>
                    {
                        self.unknown.insert(identifier.name());
                    }
                    _ => {}
                }
            }
            Expr::Field(access) => {
                let object = access.object();
                let field = access.field();
                if let Expr::Ident(prefix) = object.unparenthesized()
                    && !state.environment.is_bound(prefix.name())
                {
                    let names = QualifiedName::prefixed(prefix.name(), field.name());
                    match state.model.resolve_imported_member(names) {
                        Some(identity) if framework_identity(&identity) => {
                            self.prefixed.insert((prefix.name(), field.name()));
                        }
                        None => {
                            self.unknown.insert(prefix.name());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

struct TypeUsage<'a, 'model, 'tree> {
    model: &'a falcon_analyze::SemanticModel<'model>,
    unprefixed: &'a mut HashSet<&'tree str>,
    prefixed: &'a mut HashSet<(&'tree str, &'tree str)>,
    unknown: &'a mut HashSet<&'tree str>,
}

impl<'tree> Visitor<'tree> for TypeUsage<'_, '_, 'tree> {
    fn visit_dart_type(&mut self, node: DartType<'tree>) {
        if let DartType::Named(named) = node {
            let mut segments = named.segments().map(Identifier::name);
            match self.model.resolve_type(node).view() {
                ResolvedTypeView::Interface { identity, .. } if framework_identity(identity) => {
                    match (segments.next(), segments.next(), segments.next()) {
                        (Some(name), None, None) => {
                            self.unprefixed.insert(name);
                        }
                        (Some(prefix), Some(name), None) => {
                            self.prefixed.insert((prefix, name));
                        }
                        _ => {}
                    }
                }
                ResolvedTypeView::Unknown => {
                    if let Some(first) = segments.next() {
                        self.unknown.insert(first);
                    }
                }
                _ => {}
            }
        }
        walk_dart_type(self, node);
    }

    fn visit_annotation(&mut self, node: Annotation<'tree>) {
        if let Some(name) = QualifiedName::from_identifiers(node.name())
            && let Some(identity) = self.model.resolve_imported_member(name)
            && framework_identity(&identity)
        {
            match name.prefix {
                None => self.unprefixed.insert(name.name),
                Some(prefix) => self.prefixed.insert((prefix, name.name)),
            };
        }
        walk_annotation(self, node);
    }
}

fn framework_identity(identity: &DeclarationIdentity) -> bool {
    matches!(identity, DeclarationIdentity::Package { package, .. } if package.as_str() == "flutter")
        || matches!(identity, DeclarationIdentity::Sdk { library, .. } if library.as_str() == "dart:async")
}

fn import_targets_framework(import: ImportDirective<'_>) -> bool {
    std::iter::once(import.uri())
        .chain(import.configurable_uris().map(ConfigurableUri::uri))
        .any(|uri| uri.value().starts_with("package:flutter/") || uri.value() == "dart:async")
}

fn import_is_used(import: ImportDirective<'_>, usage: &Usage) -> bool {
    if let Some(prefix) = import.as_name() {
        return usage
            .prefixed
            .iter()
            .any(|(used_prefix, name)| *used_prefix == prefix.name() && allows(import, name))
            || usage.unknown.contains(prefix.name());
    }
    usage.unprefixed.iter().any(|name| allows(import, name))
        || usage.unknown.iter().any(|name| allows(import, name))
}

fn allows(import: ImportDirective<'_>, name: &str) -> bool {
    import.combinators().all(|combinator| match combinator {
        ImportCombinator::Show(names) => names.names().any(|shown| shown.name() == name),
        ImportCombinator::Hide(names) => names.names().all(|hidden| hidden.name() != name),
    })
}
