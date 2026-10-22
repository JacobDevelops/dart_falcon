//! The analyzer's `InterfaceElement.getInheritedConcreteMember` over falcon's
//! signature index, shared by rules that compare a declaration with the
//! member it overrides in the superclass chain.

use falcon_analyze::{
    DeclarationIdentity, MemberFacts, ResolvedType, ResolvedTypeView, SemanticModel, SignatureIndex,
};
use falcon_syntax::cst::{
    AstChildren, ClassMember, DartType, Identifier, PrimaryConstructor, TopLevelDecl,
};

/// Where a declaration's inherited concrete members come from.
pub(crate) enum InheritanceSource<'a> {
    /// Classes, enums, and mixin classes: the superclass chain and mixin
    /// applications.
    Superclasses,
    /// Mixins: the interfaces of their `on` types (`Object` without one).
    SuperclassConstraints(AstChildren<'a, DartType<'a>>),
}

/// A declaration whose instance members can override inherited ones.
pub(crate) struct InheritingDeclaration<'a> {
    pub name: Identifier<'a>,
    pub members: AstChildren<'a, ClassMember<'a>>,
    /// The header's primary constructor, whose declaring parameters are
    /// instance fields alongside `members`.
    pub primary_constructor: Option<PrimaryConstructor<'a>>,
    pub source: InheritanceSource<'a>,
}

pub(crate) fn inheriting_declaration<'a>(
    declaration: TopLevelDecl<'a>,
) -> Option<InheritingDeclaration<'a>> {
    let (name, members, primary_constructor, source) = match declaration {
        TopLevelDecl::Class(node) => (
            node.name(),
            node.members(),
            node.primary_constructor(),
            InheritanceSource::Superclasses,
        ),
        TopLevelDecl::MixinClass(node) => (
            node.name(),
            node.members(),
            node.primary_constructor(),
            InheritanceSource::Superclasses,
        ),
        TopLevelDecl::Enum(node) => (
            node.name(),
            node.members(),
            node.primary_constructor(),
            InheritanceSource::Superclasses,
        ),
        TopLevelDecl::Mixin(node) => (
            node.name(),
            node.members(),
            None,
            InheritanceSource::SuperclassConstraints(node.on_clause()),
        ),
        TopLevelDecl::ExtensionType(_)
        | TopLevelDecl::ClassTypeAlias(_)
        | TopLevelDecl::Extension(_)
        | TopLevelDecl::Function(_)
        | TopLevelDecl::Variable(_)
        | TopLevelDecl::TypeAlias(_)
        | TopLevelDecl::Error(_) => return None,
    };
    Some(InheritingDeclaration {
        name,
        members,
        primary_constructor,
        source,
    })
}

/// The non-setter member named `name` that `owner` inherits as its concrete
/// super implementation: the first concrete one in the superclass chain, or
/// for a mixin the member of its superclass constraints' interfaces. `None`
/// when there is none or the inheritance chain is unknown.
pub(crate) fn inherited_concrete_member<'a>(
    model: &SemanticModel<'_>,
    signatures: &'a SignatureIndex,
    owner: &DeclarationIdentity,
    name: &str,
    source: &InheritanceSource<'_>,
) -> Option<&'a MemberFacts> {
    let interface_member = |ty: &ResolvedType| {
        let ResolvedTypeView::Interface { identity, .. } = ty.view() else {
            return None;
        };
        signatures.interface_member(owner, identity, name)?
    };
    match source {
        InheritanceSource::Superclasses => signatures.inherited_concrete_member(owner, name)?,
        InheritanceSource::SuperclassConstraints(constraints) if constraints.is_empty() => {
            interface_member(&model.core_type("Object", false))
        }
        InheritanceSource::SuperclassConstraints(constraints) => constraints
            .clone()
            .find_map(|constraint| interface_member(&model.resolve_type(constraint))),
    }
}
