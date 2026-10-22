use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::*;

/// Members of the declarations the analyzer checks: classes (including mixin
/// classes), enums, and extension types.
pub(crate) fn constructor_container(
    declaration: TopLevelDecl<'_>,
) -> Option<AstChildren<'_, ClassMember<'_>>> {
    match declaration {
        TopLevelDecl::Class(node) => Some(node.members()),
        TopLevelDecl::MixinClass(node) => Some(node.members()),
        TopLevelDecl::Enum(node) => Some(node.members()),
        TopLevelDecl::ExtensionType(node) => Some(node.members()),
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
pub(crate) fn constructor_name_range(constructor: ConstructorDecl<'_>) -> DiagSpan {
    DiagSpan {
        start: constructor.name().span().start,
        end: constructor
            .constructor_name()
            .map_or(constructor.name().span().end, |name| name.span().end),
    }
}
