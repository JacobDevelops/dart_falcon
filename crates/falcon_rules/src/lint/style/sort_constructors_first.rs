//! Flags a constructor declared after a non-constructor member.
//!
//! Constructors describe how an instance comes to be, so they lead the member
//! list of a class, mixin class, enum, or extension type:
//!
//! ```dart
//! class A {
//!   A();
//!   int x = 0;
//!   A.named();                 // flagged
//! }
//! ```
//!
//! Each out-of-place constructor is reported at its name. Enum values are not
//! members, so they never count as preceding declarations.
//! Primary constructor headers do not participate in body ordering; their
//! explicit `this` bodies do, and are reported at the `this` keyword.
//!
//! Adopted from the official `sort_constructors_first` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct SortConstructorsFirst;

impl Rule for SortConstructorsFirst {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for members in program
            .declarations
            .iter()
            .filter_map(constructor_container)
        {
            let mut other = false;
            for member in members {
                match member {
                    ClassMember::Constructor(constructor) => {
                        if other {
                            diags.push(ctx.diagnostic(
                                "sort-constructors-first",
                                "Constructor declarations should be before non-constructor \
                         declarations.",
                                constructor_name_range(constructor),
                            ));
                        }
                    }
                    ClassMember::PrimaryConstructorBody(body) => {
                        if other {
                            diags.push(ctx.diagnostic(
                                "sort-constructors-first",
                                "Constructor declarations should be before non-constructor \
                         declarations.",
                                body.this_span,
                            ));
                        }
                    }
                    _ => other = true,
                }
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}

/// Members of the declarations the analyzer checks: classes (including mixin
/// classes), enums, and extension types.
pub(crate) fn constructor_container(declaration: &TopLevelDecl) -> Option<&[ClassMember]> {
    match declaration {
        TopLevelDecl::Class(node) => Some(&node.members),
        TopLevelDecl::MixinClass(node) => Some(&node.members),
        TopLevelDecl::Enum(node) => Some(&node.members),
        TopLevelDecl::ExtensionType(node) => Some(&node.members),
        TopLevelDecl::ClassTypeAlias(_)
        | TopLevelDecl::Mixin(_)
        | TopLevelDecl::Extension(_)
        | TopLevelDecl::Function(_)
        | TopLevelDecl::Variable(_)
        | TopLevelDecl::TypeAlias(_)
        | TopLevelDecl::Error(_) => None,
    }
}

/// The analyzer's constructor error range: `Type` or `Type.name`.
pub(crate) fn constructor_name_range(constructor: &ConstructorDecl) -> DiagSpan {
    DiagSpan {
        start: constructor.name.span.start,
        end: constructor
            .constructor_name
            .as_ref()
            .map_or(constructor.name.span.end, |name| name.span.end),
    }
}
