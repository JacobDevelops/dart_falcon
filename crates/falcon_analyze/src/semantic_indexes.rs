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

/// Refresh changed project libraries and their dependents in an existing snapshot.
///
/// The caller must retain the same external source and environment snapshot.
/// Changes to project membership or package identities rebuild the full snapshot.
/// Returns the current grouping and number of rebuilt project libraries.
pub fn update_semantic_indexes(
    indexes: &mut SemanticIndexes,
    files: &[IdentitySource<'_>],
    packages: &[PackageIdentity],
    external: Option<ExternalSources<'_>>,
    cache: Option<ExternalIndexParts<'_>>,
) -> (LibraryGrouping, usize) {
    let grouping_files: Vec<_> = files
        .iter()
        .map(|file| (file.path.to_path_buf(), file.parsed.program()))
        .collect();
    let grouping = group_libraries(&grouping_files);
    let Some(dirty) = indexes
        .identities
        .update_project(files, packages, external, &grouping)
    else {
        let (rebuilt, grouping) = build_semantic_indexes(files, packages, external, cache);
        *indexes = rebuilt;
        let libraries: std::collections::HashSet<_> = files
            .iter()
            .filter_map(|file| indexes.identities.file(file.path))
            .map(|file| indexes.identities.library(file))
            .collect();
        return (grouping, libraries.len());
    };
    indexes
        .types
        .update_project(files, &indexes.identities, &grouping, &dirty);
    for file in files {
        let id = indexes
            .identities
            .file(file.path)
            .expect("indexed project file");
        if !dirty.contains(&indexes.identities.library(id))
            && !Arc::ptr_eq(indexes.identities.parsed_file(id), file.parsed)
        {
            indexes
                .signatures
                .refresh_file_body(id, file.parsed.program());
        }
    }
    indexes
        .signatures
        .update_project(files, &indexes.identities, &indexes.types, &dirty);
    let rebuilt = files
        .iter()
        .enumerate()
        .filter(|(ordinal, file)| {
            grouping
                .siblings(*ordinal)
                .iter()
                .all(|sibling| sibling > ordinal)
                && dirty.contains(
                    &indexes.identities.library(
                        indexes
                            .identities
                            .file(file.path)
                            .expect("indexed project file"),
                    ),
                )
        })
        .count();
    (grouping, rebuilt)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::ParsedFile;

    fn parsed(source: &str) -> Arc<ParsedFile> {
        let result = falcon_dart_parser::parse(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        Arc::new(ParsedFile {
            source: source.into(),
            tree: result.tree,
        })
    }

    fn sources(files: &[(PathBuf, Arc<ParsedFile>)]) -> Vec<IdentitySource<'_>> {
        files
            .iter()
            .map(|(path, parsed)| IdentitySource {
                path,
                parsed,
                has_parse_errors: false,
            })
            .collect()
    }

    #[test]
    fn declaration_updates_match_fresh_indexes_and_retain_unrelated_shapes() {
        let initial = [
            ("/project/a.dart", "class A { int get value => 1; }"),
            ("/project/barrel.dart", "export 'a.dart';"),
            (
                "/project/consumer.dart",
                "import 'barrel.dart'; final result = A().value;",
            ),
            ("/project/part.dart", "class Piece {}"),
            (
                "/project/unrelated.dart",
                "class Unrelated { void run() {} }",
            ),
        ];
        let mut files: Vec<_> = initial
            .iter()
            .map(|(path, source)| (PathBuf::from(path), parsed(source)))
            .collect();
        let (mut indexes, _) = build_semantic_indexes(&sources(&files), &[], None, None);
        let unrelated = indexes.identities.library(
            indexes
                .identities
                .file(Path::new("/project/unrelated.dart"))
                .unwrap(),
        );
        let unrelated_identity = indexes
            .identities
            .resolve_declaration(unrelated, "Unrelated".into())
            .unwrap();
        let shape = indexes.signatures.declaration(&unrelated_identity).unwrap()
            as *const crate::DeclarationFacts;
        for (file, source) in [
            (0, "class A { String get value => 'a'; }"),
            (2, "import 'a.dart'; final result = A().value;"),
            (1, "export 'a.dart' hide A;"),
            (0, "part 'part.dart'; class A { String get value => 'a'; }"),
            (3, "part of 'a.dart'; class Piece extends A {}"),
            (0, "class A { bool get value => true; }"),
            (3, "class Piece {}"),
        ] {
            files[file].1 = parsed(source);
            let input = sources(&files);
            let (_, rebuilt) = update_semantic_indexes(&mut indexes, &input, &[], None, None);
            assert!(rebuilt < files.len(), "unrelated library was rebuilt");
            if source.starts_with("part of") {
                assert_eq!(rebuilt, 3, "retired part library is not rebuilt");
            }
            assert_eq!(
                shape,
                indexes.signatures.declaration(&unrelated_identity).unwrap()
                    as *const crate::DeclarationFacts,
            );
            let (fresh, _) = build_semantic_indexes(&input, &[], None, None);
            for (path, _) in &files {
                let library = indexes
                    .identities
                    .library(indexes.identities.file(path).unwrap());
                let fresh_library = fresh
                    .identities
                    .library(fresh.identities.file(path).unwrap());
                assert_eq!(library, fresh_library);
                for name in ["A", "Piece", "Unrelated", "result"] {
                    assert_eq!(
                        indexes.identities.resolve_declaration(library, name.into()),
                        fresh
                            .identities
                            .resolve_declaration(fresh_library, name.into()),
                    );
                    let identity = indexes
                        .identities
                        .resolve_value_declaration(library, name.into());
                    let fresh_identity = fresh
                        .identities
                        .resolve_value_declaration(fresh_library, name.into());
                    assert_eq!(identity, fresh_identity);
                    assert_eq!(
                        identity
                            .as_ref()
                            .and_then(|identity| indexes.signatures.value_type(identity)),
                        fresh_identity
                            .as_ref()
                            .and_then(|identity| fresh.signatures.value_type(identity)),
                    );
                    if let Some(identity) =
                        indexes.identities.resolve_declaration(library, name.into())
                    {
                        assert_eq!(
                            indexes.types.member_lookup_identity(&identity, "value"),
                            fresh.types.member_lookup_identity(&identity, "value"),
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "release-mode timing benchmark"]
    fn declaration_index_update_baseline() {
        use std::hint::black_box;
        use std::time::Instant;

        let mut files = vec![
            (
                PathBuf::from("/project/a.dart"),
                parsed("class A { int value = 0; }"),
            ),
            (
                PathBuf::from("/project/dependent.dart"),
                parsed("import 'a.dart'; class Dependent extends A {} final value = A().value;"),
            ),
        ];
        files.reserve(1_024);
        for ordinal in 0..1_024 {
            let source: String = (0..8)
                .map(|member| {
                    format!(
                        "class Independent{ordinal}_{member} {{ int value = 0; int run(int x) => value + x; }}\n"
                    )
                })
                .collect();
            files.push((
                PathBuf::from(format!("/project/independent{ordinal}.dart")),
                parsed(&source),
            ));
        }
        let original = Arc::clone(&files[0].1);
        let edited = parsed("class A { String value = 'changed'; }");
        let (mut indexes, _) = build_semantic_indexes(&sources(&files), &[], None, None);
        let mut incremental = Vec::with_capacity(7);
        let mut full = Vec::with_capacity(7);
        for sample in 0..7 {
            files[0].1 = Arc::clone(if sample % 2 == 0 { &edited } else { &original });
            let input = sources(&files);
            let start = Instant::now();
            let (_, rebuilt) = update_semantic_indexes(&mut indexes, &input, &[], None, None);
            incremental.push(start.elapsed());
            assert_eq!(rebuilt, 2);
            black_box(&indexes);
            let start = Instant::now();
            let fresh = build_semantic_indexes(&input, &[], None, None);
            full.push(start.elapsed());
            black_box(&fresh);
        }
        incremental.sort_unstable();
        full.sort_unstable();
        println!(
            "declaration edit: {} libraries, 7 samples, median incremental {:?}, full {:?}, speedup {:.2}x",
            files.len(),
            incremental[3],
            full[3],
            full[3].as_secs_f64() / incremental[3].as_secs_f64(),
        );
    }
}
