//! Discover Dart sources and package manifests in one pruning directory walk.

use falcon_config::PathFilter;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[cfg(test)]
thread_local! {
    static VISITED_ENTRIES: std::cell::RefCell<Vec<PathBuf>> = const {
        std::cell::RefCell::new(Vec::new())
    };
}

#[derive(Debug, Default)]
pub struct WalkOutput {
    pub files: Vec<(PathBuf, String)>,
    pub manifests: Vec<(PathBuf, String)>,
    pub missing_paths: Vec<PathBuf>,
    /// Explicit supported files rejected by the configured path filter.
    pub filtered_paths: Vec<PathBuf>,
}

/// Collect matching Dart sources and, when requested, package manifests.
///
/// Includes select Dart files only. Exclusions prune directories and apply to
/// manifests, including manifests discovered in input paths' ancestors.
/// Ordinary source read and traversal errors warn and skip; oversized sources
/// abort collection rather than analyzing a partial source.
///
/// # Errors
///
/// Returns an error if a source exceeds the size limit.
pub fn walk_project(
    paths: &[PathBuf],
    filter: &PathFilter,
    collect_manifests: bool,
) -> Result<WalkOutput, String> {
    let mut output = WalkOutput::default();
    let mut seen = HashSet::new();
    for path in paths {
        let metadata = match path.metadata() {
            Ok(metadata) => metadata,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                output.missing_paths.push(path.clone());
                continue;
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "failed to access input");
                continue;
            }
        };
        if metadata.is_file() {
            let is_dart = path.extension().and_then(|extension| extension.to_str()) == Some("dart");
            let is_manifest = collect_manifests
                && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml");
            if !is_dart && !is_manifest {
                continue;
            }
            let admitted = if is_manifest {
                !filter.excludes(&path.to_string_lossy())
            } else {
                filter.admits(&path.to_string_lossy())
            };
            if !admitted && seen.insert(path.clone()) {
                output.filtered_paths.push(path.clone());
            }
        }
        if collect_manifests
            && if metadata.is_file() {
                !filter.excludes(&path.to_string_lossy())
            } else {
                !filter.excludes_directory(path)
            }
        {
            let start = if metadata.is_file() {
                path.parent().unwrap_or(path)
            } else {
                path.as_path()
            };
            for ancestor in start.ancestors() {
                let manifest = ancestor.join("pubspec.yaml");
                if manifest.is_file() {
                    collect_file(&manifest, filter, true, &mut seen, &mut output)?;
                }
            }
        }
        if metadata.is_file() {
            collect_file(path, filter, collect_manifests, &mut seen, &mut output)?;
        } else if metadata.is_dir() {
            // Flutter plugin symlinks point into dependency caches and may cycle.
            for item in WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    !entry.file_type().is_dir() || !filter.excludes_directory(entry.path())
                })
            {
                let entry = match item {
                    Ok(entry) => entry,
                    Err(error) => {
                        tracing::warn!(%error, "failed to walk directory");
                        continue;
                    }
                };
                #[cfg(test)]
                VISITED_ENTRIES.with_borrow_mut(|visited| visited.push(entry.path().to_path_buf()));
                if entry.file_type().is_file()
                    || (entry.file_type().is_symlink() && entry.path().is_file())
                {
                    collect_file(
                        entry.path(),
                        filter,
                        collect_manifests,
                        &mut seen,
                        &mut output,
                    )?;
                }
            }
        }
    }
    output.files.sort_by(|a, b| a.0.cmp(&b.0));
    output.manifests.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(output)
}

fn collect_file(
    path: &Path,
    filter: &PathFilter,
    collect_manifests: bool,
    seen: &mut HashSet<PathBuf>,
    output: &mut WalkOutput,
) -> Result<(), String> {
    let is_manifest = collect_manifests
        && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml");
    let is_dart = path.extension().and_then(|extension| extension.to_str()) == Some("dart");
    if !is_manifest && !is_dart {
        return Ok(());
    }
    let admitted = if is_manifest {
        !filter.excludes(&path.to_string_lossy())
    } else {
        filter.admits(&path.to_string_lossy())
    };
    if !admitted || seen.contains(path) {
        return Ok(());
    }
    seen.insert(path.to_path_buf());
    if let Some(source) = read_source(path)? {
        let results = if is_manifest {
            &mut output.manifests
        } else {
            &mut output.files
        };
        results.push((path.to_path_buf(), source));
    }
    Ok(())
}

fn read_source(path: &Path) -> Result<Option<String>, String> {
    match falcon_analyze::read_source(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if falcon_analyze::is_source_too_large(&error) => {
            Err(format!("failed to read file {}: {error}", path.display()))
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read file");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_directories_are_not_traversed() {
        let directory = tempfile::tempdir().unwrap();
        let excluded = directory.path().join("build");
        std::fs::create_dir_all(excluded.join("nested")).unwrap();
        let hidden = excluded.join("nested/hidden.dart");
        std::fs::write(&hidden, "void hidden() {}").unwrap();
        let source = directory.path().join("main.dart");
        std::fs::write(&source, "void main() {}").unwrap();
        let mut filter = PathFilter::from_includes(&["**".into(), "!build".into()]);
        filter.set_base_dir(directory.path());
        VISITED_ENTRIES.with_borrow_mut(Vec::clear);
        walk_project(&[directory.path().to_path_buf()], &filter, true).unwrap();
        VISITED_ENTRIES.with_borrow(|visited| {
            assert!(visited.contains(&source));
            assert!(!visited.iter().any(|path| path.starts_with(&excluded)));
        });
    }
}
