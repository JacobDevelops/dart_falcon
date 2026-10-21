//! Flags deprecations that are not carried through to related declarations.
//!
//! Deprecating a class implies deprecating its constructors, and a deprecated
//! field implies that the initializing formal (`this.field`) that sets it is
//! deprecated too — and vice versa. Leaving one side unmarked means callers get
//! no warning on the path they actually use.
//!
//! ```dart
//! @deprecated
//! class A {
//!   A();                        // flagged: constructor not deprecated
//! }
//!
//! class B {
//!   B({this.field});            // flagged: parameter not deprecated
//!   @Deprecated('Use x')
//!   Object? field;
//! }
//! ```
//!
//! Private fields are not checked. Deprecation metadata is resolved to
//! `dart:core`, including prefixed references and named `Deprecated`
//! constructors; user-defined lookalikes do not count.
//! Primary constructors are checked separately from their enclosing type;
//! only their own body metadata deprecates them. Header initializing formals
//! participate in the same field/parameter consistency checks.
//!
//! Adopted from the official `deprecated_consistency` lint.

use falcon_analyze::{
    AnalyzeContext, QualifiedName, ResolvedType, Rule, RulePreparation, SemanticModel,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct DeprecatedConsistency;

impl Rule for DeprecatedConsistency {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let visitor = Declarations {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            diagnostics: Vec::new(),
            deprecated: false,
            fields: Vec::new(),
            declarations: program.declarations(),
        };
        RulePreparation::Structural(Box::new(visitor))
    }
}

struct Declarations<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
    deprecated: bool,
    declarations: AstChildren<'a, TopLevelDecl<'a>>,
    fields: Vec<(&'a str, AstChildren<'a, Annotation<'a>>, Option<Span>)>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Declarations<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) -> StructuralTraversal {
        // Top-level callbacks follow the program's declaration order.
        let Some(node) = self.declarations.next() else {
            return StructuralTraversal::SkipChildren;
        };
        let (annotations, elements) = match node {
            TopLevelDecl::Class(node) => (node.annotations(), node.element_members()),
            TopLevelDecl::MixinClass(node) => (node.annotations(), node.element_members()),
            TopLevelDecl::Enum(node) => (node.annotations(), node.element_members()),
            TopLevelDecl::ExtensionType(node) => (node.annotations(), node.element_members()),
            _ => return StructuralTraversal::SkipChildren,
        };
        self.deprecated = has_deprecated(annotations, self.model);
        self.fields.clear();
        for member in elements.declared.clone() {
            if let ClassMember::Field(field) = member
                && !field.is_static()
            {
                for declarator in field.declarators() {
                    self.fields.push((
                        declarator.name().name(),
                        field.annotations(),
                        Some(declarator.name().span()),
                    ));
                }
            }
        }
        for field in elements.declaring_params() {
            self.fields.push((
                field.name().name(),
                field.annotations(),
                (!elements.representation).then_some(field.name().span()),
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, _node: Expr<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_function_body(&mut self, _node: FunctionBody<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        if self.deprecated && !has_deprecated(node.annotations, self.model) {
            let end = node
                .constructor_name
                .map_or(node.name_span.end, |name| name.span().end);
            self.diagnostics.push(self.ctx.diagnostic(
                "Constructors in a deprecated class should be deprecated.",
                Span::new(node.name_span.start, end),
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, param: FormalParam<'a>) -> StructuralTraversal {
        if param.initializing() == Some(InitializingFormal::This)
            && !param.name().name().starts_with('_')
            && let Some((_, field_annotations, field_span)) = self.fields.iter()
                .find(|(name, _, _)| *name == param.name().name())
        {
            let field_deprecated = has_deprecated(field_annotations.clone(), self.model);
            let parameter_deprecated = has_deprecated(param.annotations(), self.model);
            if field_deprecated && !parameter_deprecated {
                self.diagnostics.push(self.ctx.diagnostic(
                    "Parameters that initialize a deprecated field should be deprecated.",
                    param.span(),
                ));
            } else if !field_deprecated
                && parameter_deprecated
                && let Some(field_span) = field_span
            {
                self.diagnostics.push(self.ctx.diagnostic(
                    "Fields that are initialized by a deprecated parameter should be deprecated.",
                    *field_span,
                ));
            }
        }
        StructuralTraversal::Continue
    }
}
}

fn has_deprecated<'a>(
    mut annotations: AstChildren<'a, Annotation<'a>>,
    model: SemanticModel<'_>,
) -> bool {
    annotations.any(|annotation| {
        let names = annotation.name();
        if annotation.args().is_none() {
            return annotation.constructor_name().is_none()
                && QualifiedName::from_identifiers(names).is_some_and(|name| {
                    model
                        .resolve_sdk_member(name)
                        .is_some_and(|id| id.is_sdk("dart:core", "deprecated"))
                });
        }
        let is_deprecated_type = |name: Option<QualifiedName<'_>>| {
            name.is_some_and(|name| {
                model.resolve_name(name).is_some_and(|identity| {
                    if identity.is_sdk("dart:core", "Deprecated") {
                        return true;
                    }
                    matches!(
                        model.declared_type(&identity),
                        ResolvedType::Interface { identity, .. }
                            if identity.is_sdk("dart:core", "Deprecated")
                    )
                })
            })
        };
        is_deprecated_type(QualifiedName::from_identifiers(names.clone()))
            || (annotation.constructor_name().is_none()
                && names.clone().count() > 1
                && is_deprecated_type(QualifiedName::from_identifiers(
                    names.clone().take(names.count() - 1),
                )))
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;

    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn primary_constructors_do_not_borrow_type_deprecation() {
        let source = "// @dart = 3.13\n\
            @deprecated class A();\n\
            @deprecated class B.named();\n\
            @deprecated extension type E(int value) {}\n\
            class C(this.value) { @deprecated int value; }\n\
            @deprecated class D() { @deprecated this {} }\n\
            class F(@deprecated this.value) { int value; }";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let program = parsed.program();
        let path = Path::new("/project/lib/api.dart");
        let sources = [IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::build(&sources, &[source], &[], None);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed).with_identities(&identities);
        let spans: Vec<_> = DeprecatedConsistency.analyze(DeprecatedConsistency::NAME, program, &ctx)
            .into_iter()
            .map(|d| &source[d.span.start..d.span.end])
            .collect();
        assert_eq!(spans, ["A", "B.named", "E", "this.value", "value"]);
    }
}
