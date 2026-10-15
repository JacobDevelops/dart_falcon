//! Declaration identities, canonical semantic facts, and conservative type indexes.

pub mod flow;
pub mod flow_writes;
mod identity;
mod library;
mod semantic;
mod type_index;

pub use identity::{
    DeclarationIdentity, DocImport, DocName, ExternalSources, FileId, IdentityIndex,
    IdentitySource, LibraryId, MemberSelector, MemberSelectorKind, NameIdentity, PackageIdentity,
    QualifiedName, SourceSelector, doc_imports, import_path, library_doc_imports,
};
pub use library::{
    LibraryGrouping, LibraryUnit, canonical_or_lexical, group_libraries, lexical_normalize,
    library_unit,
};
pub use semantic::{
    ConstConstructorBody, DeclarationFacts, FunctionTypeAlias, InferenceContext, MemberFacts,
    ParameterDefault, ParameterOwner, ResolvedSignature, ResolvedType, SemanticModel,
    SignatureIndex, TypeEnvironment, TypeParameterId, TypeParameterScope,
};
pub use semantic::{
    expression_has_dot_shorthand_base, expression_segments, resolve_call_constructor,
    resolve_new_constructor,
};
pub use type_index::{
    LibrarySource, MemberKind, MemberResult, TypeIndex, TypeKind, has_immutable_annotation,
};

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
