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
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_formal_param, walk_top_level_decl};

#[derive(Default)]
pub struct DeprecatedConsistency;

impl Rule for DeprecatedConsistency {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let mut visitor = Declarations {
            ctx,
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            diagnostics: Vec::new(),
        };
        visitor.visit_program(program);
        RulePreparation::Diagnostics(visitor.diagnostics)
    }
}

struct Declarations<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor for Declarations<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let (annotations, elements) = match node {
            TopLevelDecl::Class(node) => (&node.annotations, node.element_members()),
            TopLevelDecl::MixinClass(node) => (&node.annotations, node.element_members()),
            TopLevelDecl::Enum(node) => (&node.annotations, node.element_members()),
            TopLevelDecl::ExtensionType(node) => (&node.annotations, node.element_members()),
            _ => return,
        };
        let mut visitor = Members {
            ctx: self.ctx,
            model: self.model,
            deprecated: has_deprecated(annotations, self.model),
            elements,
            diagnostics: &mut self.diagnostics,
        };
        walk_top_level_decl(&mut visitor, node);
    }
}

struct Members<'a, 'b> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    deprecated: bool,
    elements: EffectiveMembers<'b>,
    diagnostics: &'b mut Vec<Diagnostic>,
}

impl Visitor for Members<'_, '_> {
    fn visit_expr(&mut self, _node: &Expr) {}

    fn visit_function_body(&mut self, _node: &FunctionBody) {}

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        if self.deprecated && !has_deprecated(node.annotations, self.model) {
            let end = node
                .constructor_name
                .as_ref()
                .map_or(node.name_span.end, |name| name.span.end);
            self.diagnostics.push(self.ctx.diagnostic(
                "deprecated-consistency",
                "Constructors in a deprecated class should be deprecated.",
                Span::new(node.name_span.start, end),
            ));
        }
    }

    fn visit_formal_param(&mut self, param: &FormalParam) {
        if param.initializing == Some(InitializingFormal::This) && !param.name.name.starts_with('_')
        {
            let field = self
                .elements
                .declared
                .iter()
                .find_map(|member| match member {
                    ClassMember::Field(field) if !field.is_static => field
                        .declarators
                        .iter()
                        .find(|declarator| declarator.name.name == param.name.name)
                        .map(|declarator| {
                            (field.annotations.as_slice(), Some(declarator.name.span))
                        }),
                    _ => None,
                })
                .or_else(|| {
                    self.elements
                        .declaring_params()
                        .find(|field| field.name.name == param.name.name)
                        // Representation fields have no reportable declaration name.
                        .map(|field| {
                            (
                                field.annotations.as_slice(),
                                (!self.elements.representation).then_some(field.name.span),
                            )
                        })
                });
            if let Some((annotations, field_span)) = field {
                let field_deprecated = has_deprecated(annotations, self.model);
                let parameter_deprecated = has_deprecated(&param.annotations, self.model);
                if field_deprecated && !parameter_deprecated {
                    self.diagnostics.push(self.ctx.diagnostic(
                        "deprecated-consistency",
                        "Parameters that initialize a deprecated field should be deprecated.",
                        param.span,
                    ));
                } else if !field_deprecated
                    && parameter_deprecated
                    && let Some(field_span) = field_span
                {
                    self.diagnostics.push(self.ctx.diagnostic(
                        "deprecated-consistency",
                        "Fields that are initialized by a deprecated parameter should be deprecated.",
                        field_span,
                    ));
                }
            }
        }
        walk_formal_param(self, param);
    }
}

fn has_deprecated(annotations: &[Annotation], model: SemanticModel<'_>) -> bool {
    annotations.iter().any(|annotation| {
        let names = annotation.name.as_slice();
        if annotation.args.is_none() {
            return annotation.constructor_name.is_none()
                && QualifiedName::from_identifiers(names).is_some_and(|name| {
                    model
                        .resolve_sdk_member(name)
                        .is_some_and(|id| id.is_sdk("dart:core", "deprecated"))
                });
        }
        let is_deprecated_type = |names: &[Identifier]| {
            QualifiedName::from_identifiers(names).is_some_and(|name| {
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
        is_deprecated_type(names)
            || (annotation.constructor_name.is_none()
                && names.len() > 1
                && is_deprecated_type(&names[..names.len() - 1]))
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
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let path = Path::new("/project/lib/api.dart");
        let sources = [IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::build(&sources, &[source], &[], None);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config, &syntax).with_identities(&identities);
        let spans: Vec<_> = DeprecatedConsistency
            .analyze(&program, &ctx)
            .into_iter()
            .map(|d| &source[d.span.start..d.span.end])
            .collect();
        assert_eq!(spans, ["A", "B.named", "E", "this.value", "value"]);
    }
}
