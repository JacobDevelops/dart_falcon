//! Requires every public property of a `Diagnosticable` class to be referenced in its debug methods.
//!
//! Flutter's diagnostics (the widget inspector, `debugDumpApp`, `toString`)
//! only show what `debugFillProperties` adds, so a public field or getter that
//! is never mentioned there — nor in `debugDescribeChildren` — is invisible
//! when debugging. A property counts as described when its name, or its
//! `debug`-prefixed variant (`debugFoo` for `foo`), appears as an identifier in
//! either method. Widget-valued properties (and `List`/`Set` of widgets) are
//! exempt because they are described as children, and so are static members,
//! private members, and overrides (a field or getter does not override an
//! inherited setter). Like the analyzer, the rule applies to classes with a
//! supertype named `Diagnosticable` declared in an unnamed library, including
//! dependency libraries. Declared and inferred property types are checked;
//! unresolved types stay silent. Declaring primary constructor parameters are
//! checked like instance fields.
//!
//! ```dart
//! class Tile extends StatelessWidget {
//!   final String title; // BAD: not described
//!
//!   @override
//!   void debugFillProperties(DiagnosticPropertiesBuilder properties) {
//!     super.debugFillProperties(properties);
//!   }
//! }
//! ```
//!
//! Adopted from the official `diagnostic_describe_all_properties` lint.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, ResolvedType, Rule, SemanticModel,
    SignatureIndex, Truth, TypeParameterScope,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body};

/// Public instance fields and getters that `Object`, `Widget`, and `State`
/// declare; a subclass member with one of these names overrides it.
const INHERITED_PROPERTIES: &[&str] = &[
    "hashCode",
    "runtimeType",
    "key",
    "widget",
    "context",
    "mounted",
];

const DEBUG_PREFIX: &str = "debug";

#[derive(Default)]
pub struct DiagnosticDescribeAllProperties;

impl Rule for DiagnosticDescribeAllProperties {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let checker = Checker {
            model: &model,
            signatures,
            ctx,
        };
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            if let TopLevelDecl::Class(class) = declaration {
                checker.check_class(class, &mut diags);
            }
        }
        diags
    }
}

struct Checker<'a> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    ctx: &'a AnalyzeContext<'a>,
}

impl Checker<'_> {
    fn check_class(&self, class: &ClassDecl, diags: &mut Vec<Diagnostic>) {
        let Some(identity) = self.model.resolve_name(&class.name.name) else {
            return;
        };
        let diagnosticable = |supertype: &DeclarationIdentity| {
            supertype.name() == "Diagnosticable"
                && self.ctx.identities.is_some_and(|identities| {
                    identities.identity_is_in_unnamed_library(supertype) == Some(true)
                })
        };
        if self
            .signatures
            .supertype_matches(&identity, &diagnosticable)
            != Truth::Yes
        {
            return;
        }
        let mut parameters = TypeParameterScope::default();
        parameters.push(&class.type_params, self.model);
        let mut properties: Vec<&Identifier> = Vec::new();
        for parameter in (EffectiveMembers {
            declared: &class.members,
            primary: class.primary_constructor.as_ref(),
            representation: false,
        }
        .declaring_params())
        {
            if self.should_describe(
                &identity,
                &parameter.name.name,
                &parameter.annotations,
                parameter.param_type.as_ref(),
                &parameters,
            ) {
                properties.push(&parameter.name);
            }
        }
        for member in &class.members {
            match member {
                ClassMember::Field(field) if !field.is_static => {
                    for declarator in &field.declarators {
                        if self.should_describe(
                            &identity,
                            &declarator.name.name,
                            &field.annotations,
                            field.field_type.as_ref(),
                            &parameters,
                        ) {
                            properties.push(&declarator.name);
                        }
                    }
                }
                ClassMember::Getter(getter)
                    if !getter.is_static
                        && self.should_describe(
                            &identity,
                            &getter.name.name,
                            &getter.annotations,
                            getter.return_type.as_ref(),
                            &parameters,
                        ) =>
                {
                    properties.push(&getter.name);
                }
                _ => {}
            }
        }
        if properties.is_empty() {
            return;
        }
        let mut described = Described(HashSet::new());
        for member in &class.members {
            if let ClassMember::Method(method) = member
                && matches!(
                    method.name.name.as_str(),
                    "debugFillProperties" | "debugDescribeChildren"
                )
                && let Some(body) = &method.body
            {
                walk_function_body(&mut described, body);
            }
        }
        for property in properties {
            if !described.0.contains(&property.name) {
                diags.push(self.ctx.diagnostic(
                    "diagnostic-describe-all-properties",
                    "The public property isn't described by either 'debugFillProperties' or 'debugDescribeChildren'.",
                    property.span,
                ));
            }
        }
    }

    /// Public, non-overriding, and of a type proven not to be a widget property.
    fn should_describe(
        &self,
        owner: &DeclarationIdentity,
        name: &str,
        annotations: &[Annotation],
        ty: Option<&DartType>,
        parameters: &TypeParameterScope,
    ) -> bool {
        if name.starts_with('_') || self.overrides(owner, name, annotations) {
            return false;
        }
        let resolved = ty
            .map(|ty| self.model.resolve_type_in(ty, parameters))
            .or_else(|| self.signatures.field(owner, name).map(|(_, ty)| ty.clone()));
        resolved.is_some_and(|ty| self.is_widget_property(&ty) == Some(false))
    }

    fn overrides(
        &self,
        owner: &DeclarationIdentity,
        name: &str,
        annotations: &[Annotation],
    ) -> bool {
        annotations.iter().any(|annotation| {
            annotation.args.is_none()
                && annotation
                    .name
                    .last()
                    .is_some_and(|segment| segment.name == "override")
        }) || match self.signatures.inherited_member_facts(owner, name) {
            // A field or getter overrides an inherited member of its name
            // unless that member is only a setter.
            Some(facts) => facts.iter().any(|fact| fact.kind != MemberKind::Setter),
            None => INHERITED_PROPERTIES.contains(&name),
        }
    }

    /// Whether `ty` is a widget, or a `List`/`Set`/`LinkedHashSet` of widget
    /// properties; `None` when the type cannot be classified.
    fn is_widget_property(&self, ty: &ResolvedType) -> Option<bool> {
        match ty {
            ResolvedType::Unknown => None,
            ResolvedType::Interface {
                identity,
                arguments,
                ..
            } => {
                if self.signatures.is_subtype_of(
                    ty,
                    &DeclarationIdentity::flutter("Widget"),
                    self.model,
                ) == Truth::Yes
                {
                    return Some(true);
                }
                let collection = identity.is_sdk("dart:core", "List")
                    || identity.is_sdk("dart:core", "Set")
                    || identity.is_sdk("dart:collection", "LinkedHashSet");
                match identity {
                    _ if collection => match arguments.as_ref() {
                        [element] => self.is_widget_property(element),
                        _ => Some(false),
                    },
                    DeclarationIdentity::Sdk { .. } | DeclarationIdentity::Package { .. } => {
                        Some(false)
                    }
                    DeclarationIdentity::Project { .. } => {
                        let widget = |supertype: &DeclarationIdentity| match supertype {
                            DeclarationIdentity::Project { name, .. } => name == "Widget",
                            other => other.is_package("flutter", "Widget"),
                        };
                        match self.signatures.supertype_matches(identity, &widget) {
                            Truth::Yes => Some(true),
                            Truth::No => Some(false),
                            Truth::Unknown => self
                                .ctx
                                .types
                                .is_some_and(|types| {
                                    types.supertype_matches_identity(identity, widget) == Truth::No
                                })
                                .then_some(false),
                        }
                    }
                }
            }
            ResolvedType::Dynamic
            | ResolvedType::Void
            | ResolvedType::Never
            | ResolvedType::Null
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::NonNullTypeParameter { .. }
            | ResolvedType::Function { .. }
            | ResolvedType::Record { .. } => Some(false),
        }
    }
}

/// The property names described by the identifiers of a debug method: each
/// identifier `foo` describes `foo` and `debugFoo`, and each `debugFoo`
/// describes `debugFoo` and `foo`.
struct Described(HashSet<String>);

impl Described {
    fn reference(&mut self, name: &str) {
        self.0.insert(name.to_string());
        match name.strip_prefix(DEBUG_PREFIX) {
            Some(rest) if !rest.is_empty() => {
                self.0.insert(change_first(rest, char::to_lowercase));
            }
            _ => {
                self.0.insert(format!(
                    "{DEBUG_PREFIX}{}",
                    change_first(name, char::to_uppercase)
                ));
            }
        }
    }
}

fn change_first<I: Iterator<Item = char>>(text: &str, change: impl Fn(char) -> I) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |first| change(first).chain(chars).collect())
}

impl Visitor for Described {
    fn visit_identifier(&mut self, node: &Identifier) {
        self.reference(&node.name);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Field { field, .. } => self.reference(&field.name),
            Expr::Cascade { sections, .. } => {
                for op in sections.iter().flat_map(|section| &section.ops) {
                    if let CascadeOp::Field { name, .. } | CascadeOp::Call(name, _, _) = op {
                        self.reference(&name.name);
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
