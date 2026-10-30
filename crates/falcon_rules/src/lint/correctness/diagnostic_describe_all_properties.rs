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
    AnalyzeContext, DeclarationIdentity, MemberKind, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
    SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
    TypeParameterScope, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

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
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        RulePreparation::Structural(Box::new(Collector {
            checker: Checker {
                model,
                signatures,
                ctx,
            },
            declarations: program.declarations(),
            properties: Vec::new(),
            described: Described(HashSet::new()),
            debug_method: false,
            body_depth: 0,
            diags: Vec::new(),
        }))
    }
}

struct Checker<'a> {
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    ctx: &'a AnalyzeContext<'a>,
}

impl Checker<'_> {
    fn properties<'n>(&self, class: ClassDecl<'n>) -> Vec<Identifier<'n>> {
        let Some(identity) = self.model.resolve_name(class.name().name()) else {
            return Vec::new();
        };
        let diagnosticable = |supertype: &DeclarationIdentity| {
            supertype.name() == "Diagnosticable"
                && self.ctx.identities().is_some_and(|identities| {
                    identities.identity_is_in_unnamed_library(supertype) == Some(true)
                })
        };
        if self
            .signatures
            .supertype_matches(&identity, &diagnosticable)
            != Truth::Yes
        {
            return Vec::new();
        }
        let mut parameters = TypeParameterScope::default();
        parameters.push(class.type_params(), &self.model);
        let mut properties = Vec::new();
        for parameter in (EffectiveMembers {
            declared: class.members(),
            primary: class.primary_constructor(),
            representation: false,
        }
        .declaring_params())
        {
            if self.should_describe(
                &identity,
                parameter.name().name(),
                parameter.annotations(),
                parameter.param_type(),
                &parameters,
            ) {
                properties.push(parameter.name());
            }
        }
        for member in class.members() {
            match member {
                ClassMember::Field(field) if !field.is_static() => {
                    for declarator in field.declarators() {
                        if self.should_describe(
                            &identity,
                            declarator.name().name(),
                            field.annotations(),
                            field.field_type(),
                            &parameters,
                        ) {
                            properties.push(declarator.name());
                        }
                    }
                }
                ClassMember::Getter(getter)
                    if !getter.is_static()
                        && self.should_describe(
                            &identity,
                            getter.name().name(),
                            getter.annotations(),
                            getter.return_type(),
                            &parameters,
                        ) =>
                {
                    properties.push(getter.name());
                }
                _ => {}
            }
        }
        properties
    }

    /// Public, non-overriding, and of a type proven not to be a widget property.
    fn should_describe(
        &self,
        owner: &DeclarationIdentity,
        name: &str,
        annotations: AstChildren<'_, Annotation<'_>>,
        ty: Option<DartType<'_>>,
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
        annotations: AstChildren<'_, Annotation<'_>>,
    ) -> bool {
        crate::lint::support::annotations::has_override(annotations, self.model)
            || match self.signatures.inherited_member_facts(owner, name) {
            // A field or getter overrides an inherited member of its name
            // unless that member is only a setter.
            Some(facts) => facts.iter().any(|fact| fact.kind != MemberKind::Setter),
            None => INHERITED_PROPERTIES.contains(&name),
        }
    }

    /// Whether `ty` is a widget, or a `List`/`Set`/`LinkedHashSet` of widget
    /// properties; `None` when the type cannot be classified.
    fn is_widget_property(&self, ty: &ResolvedType) -> Option<bool> {
        match ty.view() {
            ResolvedTypeView::Unknown => None,
            ResolvedTypeView::Interface {
                identity,
                arguments,
                ..
            } => {
                if self.signatures.relation(ty, TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::flutter("Widget")), &self.model) == Truth::Yes
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
                    DeclarationIdentity::Library { .. } => {
                        let widget = |supertype: &DeclarationIdentity| match supertype {
                            _ if self.model.identities().is_project(supertype) => {
                                supertype.name() == "Widget"
                            }
                            other => other.is_package("flutter", "Widget"),
                        };
                        match self.signatures.supertype_matches(identity, &widget) {
                            Truth::Yes => Some(true),
                            Truth::No => Some(false),
                            Truth::Unknown => self
                                .ctx
                                .types()
                                .is_some_and(|types| {
                                    types.supertype_matches_identity(identity, widget) == Truth::No
                                })
                                .then_some(false),
                        }
                    }
                }
            }
            ResolvedTypeView::Dynamic
            | ResolvedTypeView::Void
            | ResolvedTypeView::Never
            | ResolvedTypeView::Null
            | ResolvedTypeView::TypeParameter { .. }
            | ResolvedTypeView::NonNullTypeParameter { .. }
            | ResolvedTypeView::Function { .. }
            | ResolvedTypeView::Record { .. } => Some(false),
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

impl Described {
    fn expression(&mut self, node: Expr<'_>) {
        match node {
            Expr::Field(field) => self.reference(field.field().name()),
            Expr::Cascade(cascade) => {
                for op in cascade.sections().flat_map(CascadeSection::ops) {
                    match op {
                        CascadeOp::Field(field) => self.reference(field.name().name()),
                        CascadeOp::Call(call) => self.reference(call.name().name()),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

struct Collector<'a> {
    checker: Checker<'a>,
    declarations: AstChildren<'a, TopLevelDecl<'a>>,
    properties: Vec<Identifier<'a>>,
    described: Described,
    debug_method: bool,
    body_depth: usize,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_class_decl(&mut self, _node: ClassDecl<'a>) -> StructuralTraversal {
        let class = self.declarations.find_map(|declaration| match declaration {
            TopLevelDecl::Class(class) => Some(class),
            _ => None,
        }).unwrap();
        self.properties = self.checker.properties(class);
        self.described.0.clear();
        if self.properties.is_empty() {
            StructuralTraversal::SkipChildren
        } else {
            StructuralTraversal::Continue
        }
    }

    fn visit_method_decl(&mut self, method: MethodDecl<'a>) -> StructuralTraversal {
        self.debug_method = matches!(method.name().name(), "debugFillProperties" | "debugDescribeChildren");
        StructuralTraversal::Continue
    }

    fn exit_method_decl(&mut self, _method: MethodDecl<'a>) {
        self.debug_method = false;
    }

    fn visit_function_body(&mut self, _body: FunctionBody<'a>) -> StructuralTraversal {
        self.body_depth += 1;
        StructuralTraversal::Continue
    }

    fn exit_function_body(&mut self, _body: FunctionBody<'a>) {
        self.body_depth -= 1;
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        if self.debug_method && self.body_depth > 0 {
            self.described.reference(node.name());
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if self.debug_method && self.body_depth > 0 {
            self.described.expression(node);
        }
        StructuralTraversal::Continue
    }

    fn exit_class_decl(&mut self, _node: ClassDecl<'a>) {
        for property in self.properties.drain(..) {
            if !self.described.0.contains(property.name()) {
                self.diags.push(self.checker.ctx.diagnostic(
                    "The public property isn't described by either `debugFillProperties` or `debugDescribeChildren`.",
                    property.span(),
                ));
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
