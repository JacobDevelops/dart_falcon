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
//! Private fields are not checked. Only the `dart:core` `@deprecated` constant
//! and the unnamed `@Deprecated(...)` constructor count as deprecations; a file
//! that declares its own `deprecated` or `Deprecated` is skipped.
//!
//! Adopted from the official `deprecated_consistency` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct DeprecatedConsistency;

impl Rule for DeprecatedConsistency {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if declares_deprecated(program) {
            return Vec::new();
        }
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (annotations, members) = match decl {
                TopLevelDecl::Class(node) => (&node.annotations, &node.members),
                TopLevelDecl::MixinClass(node) => (&node.annotations, &node.members),
                TopLevelDecl::Enum(node) => (&node.annotations, &node.members),
                TopLevelDecl::ExtensionType(node) => (&node.annotations, &node.members),
                _ => continue,
            };
            let class_deprecated = deprecation(annotations) == Deprecation::Plain;
            for member in members {
                let ClassMember::Constructor(constructor) = member else {
                    continue;
                };
                if class_deprecated && deprecation(&constructor.annotations) == Deprecation::None {
                    let end = constructor
                        .constructor_name
                        .as_ref()
                        .map_or(constructor.name.span.end, |name| name.span.end);
                    diags.push(ctx.diagnostic(
                        "deprecated-consistency",
                        "Constructors in a deprecated class should be deprecated.",
                        Span::new(constructor.name.span.start, end),
                    ));
                }
                for param in &constructor.params.params {
                    if param.initializing == Some(InitializingFormal::This) {
                        check_field_formal(param, members, ctx, &mut diags);
                    }
                }
            }
        }
        diags
    }
}

fn check_field_formal(
    param: &FormalParam,
    members: &[ClassMember],
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let name = &param.name.name;
    if name.starts_with('_') {
        return;
    }
    let Some((field, declarator)) = members.iter().find_map(|member| match member {
        ClassMember::Field(field) if !field.is_static => field
            .declarators
            .iter()
            .find(|declarator| declarator.name.name == *name)
            .map(|declarator| (field, declarator)),
        _ => None,
    }) else {
        return;
    };
    match (
        deprecation(&field.annotations),
        deprecation(&param.annotations),
    ) {
        (Deprecation::Plain, Deprecation::None) => diags.push(ctx.diagnostic(
            "deprecated-consistency",
            "Parameters that initialize a deprecated field should be deprecated.",
            param.span,
        )),
        (Deprecation::None, Deprecation::Plain) => diags.push(ctx.diagnostic(
            "deprecated-consistency",
            "Fields that are initialized by a deprecated parameter should be deprecated.",
            declarator.name.span,
        )),
        _ => {}
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Deprecation {
    None,
    /// `@deprecated` or `@Deprecated(...)`.
    Plain,
    /// A prefixed or named-constructor form whose meaning is not proven.
    Other,
}

fn deprecation(annotations: &[Annotation]) -> Deprecation {
    let mut result = Deprecation::None;
    for annotation in annotations {
        match annotation.name.as_slice() {
            [name]
                if annotation.constructor_name.is_none()
                    && matches!(name.name.as_str(), "deprecated" | "Deprecated") =>
            {
                return Deprecation::Plain;
            }
            segments
                if segments.iter().any(|segment| {
                    matches!(segment.name.as_str(), "deprecated" | "Deprecated")
                }) =>
            {
                result = Deprecation::Other;
            }
            _ => {}
        }
    }
    result
}

/// A local `deprecated` / `Deprecated` declaration shadows the `dart:core` one.
fn declares_deprecated(program: &Program) -> bool {
    let shadows = |name: &str| matches!(name, "deprecated" | "Deprecated");
    program.declarations.iter().any(|decl| match decl {
        TopLevelDecl::Class(node) => shadows(&node.name.name),
        TopLevelDecl::ClassTypeAlias(node) => shadows(&node.name.name),
        TopLevelDecl::Mixin(node) => shadows(&node.name.name),
        TopLevelDecl::MixinClass(node) => shadows(&node.name.name),
        TopLevelDecl::Enum(node) => shadows(&node.name.name),
        TopLevelDecl::ExtensionType(node) => shadows(&node.name.name),
        TopLevelDecl::TypeAlias(node) => shadows(&node.name.name),
        TopLevelDecl::Function(node) => shadows(&node.name.name),
        TopLevelDecl::Variable(node) => node
            .declarators
            .iter()
            .any(|declarator| shadows(&declarator.name.name)),
        TopLevelDecl::Extension(_) | TopLevelDecl::Error(_) => false,
    })
}
