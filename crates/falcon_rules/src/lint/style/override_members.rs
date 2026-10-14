use falcon_syntax::ast::{ClassMember, Program, TopLevelDecl};

pub(super) fn override_member_owners(
    program: &Program,
) -> impl Iterator<Item = (&str, &[ClassMember])> {
    program.declarations.iter().filter_map(|decl| match decl {
        TopLevelDecl::Class(node) => Some((node.name.name.as_str(), node.members.as_slice())),
        TopLevelDecl::Mixin(node) => Some((node.name.name.as_str(), node.members.as_slice())),
        TopLevelDecl::MixinClass(node) => Some((node.name.name.as_str(), node.members.as_slice())),
        TopLevelDecl::Enum(node) => Some((node.name.name.as_str(), node.members.as_slice())),
        TopLevelDecl::ExtensionType(node) => {
            Some((node.name.name.as_str(), node.members.as_slice()))
        }
        TopLevelDecl::ClassTypeAlias(_)
        | TopLevelDecl::Extension(_)
        | TopLevelDecl::Function(_)
        | TopLevelDecl::Variable(_)
        | TopLevelDecl::TypeAlias(_)
        | TopLevelDecl::Error(_) => None,
    })
}
