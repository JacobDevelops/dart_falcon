//! Declaration identities, canonical semantic facts, and conservative type indexes.

mod identity;
mod library;
mod semantic;
mod type_index;

pub use identity::{
    DeclarationIdentity, FileId, IdentityIndex, IdentitySource, LibraryId, NameIdentity,
    PackageIdentity, QualifiedName, import_path,
};
pub use library::{
    LibraryGrouping, LibraryUnit, canonical_or_lexical, group_libraries, library_unit,
};
pub use semantic::{
    DeclarationFacts, MemberFacts, ParameterOwner, ResolvedSignature, ResolvedType, SemanticModel,
    SignatureIndex, TypeEnvironment, TypeParameterId, TypeParameterScope,
};
pub use semantic::{expression_segments, resolve_call_constructor};
pub use type_index::{LibrarySource, MemberKind, MemberResult, TypeIndex, TypeKind};

/// A three-valued proof: `No` means proven false, never "not proven true".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    Yes,
    No,
    Unknown,
}

impl From<bool> for Truth {
    fn from(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}
