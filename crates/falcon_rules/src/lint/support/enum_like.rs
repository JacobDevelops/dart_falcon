//! Resolved enum-like class classification.

use falcon_analyze::{DeclarationFacts, DeclarationIdentity, ResolvedType, ResolvedTypeView};

pub(crate) fn exact_identity(ty: &ResolvedType, expected: &DeclarationIdentity) -> bool {
    matches!(ty.view(), ResolvedTypeView::Interface { identity, arguments, nullable: false, .. }
        if identity == expected && arguments.is_empty())
}
pub(crate) fn is_enum_like(facts: &DeclarationFacts, identity: &DeclarationIdentity) -> bool {
    !facts.is_abstract
        && !facts.has_subclass_in_library
        && !facts.constructors.is_empty()
        && facts
            .constructors
            .iter()
            .all(|constructor| constructor.is_private && !constructor.is_factory)
        && facts
            .static_consts
            .iter()
            .filter(|constant| exact_identity(&constant.ty, identity))
            .count()
            >= 2
}
