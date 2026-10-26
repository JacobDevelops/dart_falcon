//! Staged semantic index construction shared by project analysis consumers.

use std::sync::Arc;

use crate::{
    ExternalSources, IdentityIndex, IdentitySource, LibraryGrouping, LibrarySource,
    PackageIdentity, SemanticIndexes, SignatureIndex, TypeIndex, group_libraries,
};

/// Borrowed external-only indexes retained by the caller between project overlays.
/// All three parts must describe the same external source and environment snapshot.
#[derive(Clone, Copy)]
pub struct ExternalIndexParts<'a> {
    pub identities: &'a Arc<IdentityIndex>,
    pub types: &'a Arc<TypeIndex>,
    pub signatures: &'a Arc<SignatureIndex>,
}

/// Group sources, then build identities, types, and signatures in dependency order.
///
/// Files retain their input order, including any promoted library companions.
/// Source loading, parsing, companion discovery, and package discovery belong to
/// the caller. Cached parts must be external-only indexes for `external`.
pub fn build_semantic_indexes(
    files: &[IdentitySource<'_>],
    packages: &[PackageIdentity],
    external: Option<ExternalSources<'_>>,
    cache: Option<ExternalIndexParts<'_>>,
) -> (SemanticIndexes, LibraryGrouping) {
    let grouping_files: Vec<_> = files
        .iter()
        .map(|file| (file.path.to_path_buf(), file.parsed.program()))
        .collect();
    let grouping = group_libraries(&grouping_files);
    let sources: Vec<_> = files
        .iter()
        .map(|file| file.parsed.source.as_ref())
        .collect();
    let identities = IdentityIndex::build_layer(
        files,
        &sources,
        packages,
        external,
        cache.map(|cache| Arc::clone(cache.identities)),
        Some(&grouping),
    );
    tracing::debug!("identity index built");
    let resolved_files = || {
        files.iter().enumerate().map(|(index, file)| {
            (
                file.path,
                LibrarySource {
                    program: file.parsed.program(),
                    has_parse_errors: file.has_parse_errors,
                    has_unresolved_parts: grouping.is_unresolved(index),
                },
            )
        })
    };
    let mut types = if let Some(cache) = cache {
        TypeIndex::with_external(resolved_files(), &identities, Arc::clone(cache.types))
    } else {
        TypeIndex::from_resolved_files(resolved_files(), &identities)
    };
    if cache.is_none()
        && let Some(external) = external
    {
        types.add_external(external.libraries, &identities);
    }
    tracing::debug!("type index built");
    let signature_files: Vec<_> = grouping_files
        .into_iter()
        .zip(files)
        .map(|((path, _), file)| (path, Arc::clone(file.parsed)))
        .collect();
    let signatures = if let Some(cache) = cache {
        SignatureIndex::with_external(
            &signature_files,
            &identities,
            &types,
            Arc::clone(cache.signatures),
        )
    } else {
        SignatureIndex::build(
            &signature_files,
            external.map(|external| external.libraries),
            &identities,
            &types,
        )
    };
    tracing::debug!("semantic indexes built");
    (
        SemanticIndexes {
            types,
            identities,
            signatures,
        },
        grouping,
    )
}
