//! File walker: discovers `.dart` files recursively using a compiled path filter.

use falcon_config::PathFilter;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Walk `paths` and return matching `.dart` files with their contents.
///
/// Directories are walked recursively. Ordinary I/O errors warn and skip files;
/// oversized sources abort collection rather than analyzing a partial project.
pub fn walk_files(
    paths: &[PathBuf],
    filter: &PathFilter,
) -> Result<Vec<(PathBuf, String)>, String> {
    let mut results = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if let Some(entry) = try_read_dart(path, filter)? {
                results.push(entry);
            }
        } else if path.is_dir() {
            // Never follow symlinks: Flutter's ephemeral/.plugin_symlinks point
            // into the pub cache, turning a project walk into a multi-minute
            // crawl of every dependency (and risking cycles).
            for item in WalkDir::new(path).follow_links(false) {
                let entry = match item {
                    Ok(entry) => entry,
                    Err(error) => {
                        tracing::warn!(%error, "error walking directory");
                        continue;
                    }
                };
                if let Some(entry) = try_read_dart(entry.path(), filter)? {
                    results.push(entry);
                }
            }
        }
    }
    Ok(results)
}

/// Walk input directories and their ancestors for package manifests used by
/// pubspec-backed rules. Exclusions apply, but Dart includes never hide manifests.
pub fn walk_pubspecs(
    paths: &[PathBuf],
    filter: &PathFilter,
) -> Result<Vec<(PathBuf, String)>, String> {
    let mut manifests = Vec::new();
    for path in paths {
        let start = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path.as_path()
        };
        manifests.extend(
            start
                .ancestors()
                .map(|ancestor| ancestor.join("pubspec.yaml"))
                .filter(|manifest| manifest.is_file()),
        );

        if path.is_dir() {
            for entry in WalkDir::new(path).follow_links(false) {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        tracing::warn!(%error, "error walking directory");
                        continue;
                    }
                };
                if entry.file_name().to_str() == Some("pubspec.yaml") {
                    manifests.push(entry.into_path());
                }
            }
        } else if path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml") {
            manifests.push(path.clone());
        }
    }
    manifests.sort();
    manifests.dedup();
    manifests
        .into_iter()
        .filter(|path| !filter.excludes(&path.to_string_lossy()))
        .filter_map(|path| {
            read_source(&path)
                .transpose()
                .map(|result| result.map(|source| (path, source)))
        })
        .collect()
}

fn is_dart(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("dart")
}

fn try_read_dart(path: &Path, filter: &PathFilter) -> Result<Option<(PathBuf, String)>, String> {
    if !is_dart(path) || !filter.admits(&path.to_string_lossy()) {
        return Ok(None);
    }
    read_source(path).map(|source| source.map(|contents| (path.to_path_buf(), contents)))
}

fn read_source(path: &Path) -> Result<Option<String>, String> {
    match falcon_analyze::source::read_source(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if falcon_analyze::source::is_source_too_large(&error) => {
            Err(format!("failed to read file {}: {error}", path.display()))
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read file");
            Ok(None)
        }
    }
}
