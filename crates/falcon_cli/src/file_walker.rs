//! File walker: discovers `.dart` files recursively using a compiled path filter.

use falcon_config::PathFilter;
use std::path::{Path, PathBuf};
use tracing::warn;
use walkdir::WalkDir;

/// Walk `paths` and return matching `.dart` files with their contents.
///
/// Directories are walked recursively. Unreadable files, broken symlinks,
/// and permission errors are logged as warnings and skipped.
pub fn walk_files(paths: &[PathBuf], filter: &PathFilter) -> Vec<(PathBuf, String)> {
    let mut results = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if let Some(entry) = try_read_dart(path, filter) {
                results.push(entry);
            }
        } else if path.is_dir() {
            // Never follow symlinks: Flutter's ephemeral/.plugin_symlinks point
            // into the pub cache, turning a project walk into a multi-minute
            // crawl of every dependency (and risking cycles).
            for item in WalkDir::new(path).follow_links(false) {
                match item {
                    Ok(e) => {
                        if let Some(entry) = try_read_dart(e.path(), filter) {
                            results.push(entry);
                        }
                    }
                    Err(e) => warn!("error walking directory: {}", e),
                }
            }
        }
    }
    results
}

/// Walk input directories and their ancestors for package manifests used by
/// pubspec-backed rules. Exclusions apply, but Dart includes never hide manifests.
pub fn walk_pubspecs(paths: &[PathBuf], filter: &PathFilter) -> Vec<(PathBuf, String)> {
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
            manifests.extend(
                WalkDir::new(path)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(Result::ok)
                    .map(|entry| entry.into_path())
                    .filter(|entry| {
                        entry.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml")
                    }),
            );
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
            std::fs::read_to_string(&path)
                .ok()
                .map(|source| (path, source))
        })
        .collect()
}

fn is_dart(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("dart")
}

fn try_read_dart(path: &Path, filter: &PathFilter) -> Option<(PathBuf, String)> {
    if !is_dart(path) || !filter.admits(&path.to_string_lossy()) {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(contents) => Some((path.to_path_buf(), contents)),
        Err(_) => {
            warn!("failed to read file: {}", path.display());
            None
        }
    }
}
