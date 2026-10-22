//! Declaration identities, canonical semantic facts, and conservative type indexes.

pub mod flow;
pub mod flow_writes;
mod hierarchy;
mod identity;
pub(crate) mod lexical_scope;
pub mod lexical_usage;
mod library;
mod sdk_hierarchy;
mod semantic;
mod type_index;

pub use identity::{
    DeclarationIdentity, DocName, ExternalSources, FileId, IdentityIndex, IdentitySource,
    LibraryId, MemberSelector, MemberSelectorKind, NameIdentity, PackageIdentity, QualifiedName,
    SourceSelector, doc_imports, import_path, library_doc_imports,
};
pub use library::{
    LibraryGrouping, LibraryUnit, canonical_or_lexical, group_libraries, is_generated_dart_file,
    lexical_normalize, library_unit,
};
pub use semantic::{
    ConstConstructorBody, DeclarationFacts, FunctionTypeShape, InferenceContext, MemberFacts,
    ParameterContract, ParameterDefault, ParameterOwner, ResolvedInvocation, ResolvedSignature,
    ResolvedType, SemanticModel, SignatureIndex, TypeEnvironment, TypeParameterId,
    TypeParameterScope,
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
