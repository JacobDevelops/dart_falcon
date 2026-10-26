//! Discover Dart sources and package manifests in one pruning directory walk.

use falcon_config::PathFilter;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

#[cfg(test)]
thread_local! {
    static VISITED_ENTRIES: std::cell::RefCell<Vec<PathBuf>> = const {
        std::cell::RefCell::new(Vec::new())
    };
}

/// A pre-traversal metadata snapshot and admitted immediate directory names.
#[derive(Debug, PartialEq, Eq)]
pub struct DiscoveryWatch {
    pub path: PathBuf,
    pub modified: Option<SystemTime>,
    /// Sorted raw names with directory and symlink flags.
    pub entries: Option<Vec<(OsString, bool, bool)>>,
}

impl DiscoveryWatch {
    /// Refresh only changed directories; other metadata changes invalidate discovery.
    pub fn refresh(&mut self, filter: &PathFilter, collect_manifests: bool) -> bool {
        let metadata = self.path.metadata().ok();
        let modified = metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok());
        if modified == self.modified {
            return true;
        }
        let Some(expected) = &self.entries else {
            return false;
        };
        // Cached project paths canonicalize sources; a same-name symlink can
        // point elsewhere after replacement even when membership is unchanged.
        if expected.iter().any(|(_, _, symlink)| *symlink) {
            return false;
        }
        if !metadata.is_some_and(|metadata| metadata.is_dir()) {
            return false;
        }
        let Ok(directory) = std::fs::read_dir(&self.path) else {
            return false;
        };
        let mut entries = Vec::with_capacity(expected.len());
        for entry in directory {
            let Ok(entry) = entry else {
                return false;
            };
            let Ok(kind) = entry.file_type() else {
                return false;
            };
            let path = entry.path();
            if kind.is_dir() {
                if !filter.excludes_directory(&path) {
                    entries.push((entry.file_name(), true, false));
                }
            } else if (kind.is_file() || (kind.is_symlink() && path.is_file()))
                && admitted_file(&path, filter, collect_manifests)
            {
                entries.push((entry.file_name(), false, kind.is_symlink()));
            }
        }
        entries.sort_unstable();
        if &entries != expected {
            return false;
        }
        self.modified = modified;
        true
    }
}

/// Paths discovered without opening source or manifest contents.
#[derive(Debug)]
pub struct ProjectDiscovery {
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    /// Metadata snapshots captured before traversing watched directories.
    /// Empty unless discovery explicitly requests watches.
    pub watches: Vec<DiscoveryWatch>,
    pub missing_paths: Vec<PathBuf>,
    pub filtered_paths: Vec<PathBuf>,
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
    let discovery = discover_project(paths, filter, collect_manifests);
    let mut output = WalkOutput {
        missing_paths: discovery.missing_paths,
        filtered_paths: discovery.filtered_paths,
        ..WalkOutput::default()
    };
    for (paths, sources) in [
        (discovery.files, &mut output.files),
        (discovery.manifests, &mut output.manifests),
    ] {
        for path in paths {
            if let Some(source) = read_source(&path)? {
                sources.push((path, source));
            }
        }
    }
    Ok(output)
}

/// Discover supported paths, pruning exclusions without reading file contents.
pub fn discover_project(
    paths: &[PathBuf],
    filter: &PathFilter,
    collect_manifests: bool,
) -> ProjectDiscovery {
    discover_project_inner(paths, filter, collect_manifests, false)
}

/// Discover paths and snapshot inputs, traversed directories, and ancestor
/// manifest candidates, including missing paths, without reading file contents.
pub fn discover_project_with_watches(
    paths: &[PathBuf],
    filter: &PathFilter,
    collect_manifests: bool,
) -> ProjectDiscovery {
    discover_project_inner(paths, filter, collect_manifests, true)
}

fn discover_project_inner(
    paths: &[PathBuf],
    filter: &PathFilter,
    collect_manifests: bool,
    track_watches: bool,
) -> ProjectDiscovery {
    let mut output = ProjectDiscovery {
        files: Vec::new(),
        manifests: Vec::new(),
        watches: Vec::new(),
        missing_paths: Vec::new(),
        filtered_paths: Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut watched = track_watches.then(HashMap::new);
    let mut watches = Vec::new();
    for path in paths {
        let metadata = path.metadata();
        record_watch(path, &mut watched, &mut watches, || {
            metadata
                .as_ref()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
        });
        let metadata = match metadata {
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
                record_watch(&manifest, &mut watched, &mut watches, || {
                    manifest
                        .metadata()
                        .ok()
                        .and_then(|metadata| metadata.modified().ok())
                });
                if manifest.is_file() {
                    collect_file(&manifest, filter, true, &mut seen, &mut output);
                }
            }
        }
        if metadata.is_file() {
            collect_file(path, filter, collect_manifests, &mut seen, &mut output);
        } else if metadata.is_dir() {
            // Flutter plugin symlinks point into dependency caches and may cycle.
            for item in WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    let kind = entry.file_type();
                    if !kind.is_dir() {
                        if track_watches
                            && (kind.is_file() || (kind.is_symlink() && entry.path().is_file()))
                            && admitted_file(entry.path(), filter, collect_manifests)
                        {
                            record_entry(entry, &watched, &mut watches);
                        }
                        return true;
                    }
                    if filter.excludes_directory(entry.path()) {
                        return false;
                    }
                    record_entry(entry, &watched, &mut watches);
                    let index = record_watch(entry.path(), &mut watched, &mut watches, || {
                        entry
                            .path()
                            .metadata()
                            .ok()
                            .and_then(|metadata| metadata.modified().ok())
                    });
                    if let Some(index) = index {
                        watches[index].entries.get_or_insert_with(Vec::new);
                    }
                    true
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
                    );
                }
            }
        }
    }
    output.files.sort();
    output.manifests.sort();
    for watch in &mut watches {
        if let Some(entries) = &mut watch.entries {
            entries.sort_unstable();
            entries.dedup();
        }
    }
    output.watches = watches;
    output
}

fn record_watch(
    path: &Path,
    seen: &mut Option<HashMap<PathBuf, usize>>,
    watches: &mut Vec<DiscoveryWatch>,
    modified: impl FnOnce() -> Option<SystemTime>,
) -> Option<usize> {
    let seen = seen.as_mut()?;
    if let Some(&index) = seen.get(path) {
        return Some(index);
    }
    let index = watches.len();
    seen.insert(path.to_path_buf(), index);
    watches.push(DiscoveryWatch {
        path: path.to_path_buf(),
        modified: modified(),
        entries: None,
    });
    Some(index)
}

fn record_entry(
    entry: &walkdir::DirEntry,
    watched: &Option<HashMap<PathBuf, usize>>,
    watches: &mut [DiscoveryWatch],
) {
    if let Some(watched) = watched
        && let Some(parent) = entry.path().parent()
        && let Some(&index) = watched.get(parent)
        && let Some(entries) = &mut watches[index].entries
    {
        entries.push((
            entry.file_name().to_os_string(),
            entry.file_type().is_dir(),
            entry.file_type().is_symlink(),
        ));
    }
}

fn admitted_file(path: &Path, filter: &PathFilter, collect_manifests: bool) -> bool {
    if collect_manifests && path.file_name().is_some_and(|name| name == "pubspec.yaml") {
        !filter.excludes(&path.to_string_lossy())
    } else {
        path.extension()
            .is_some_and(|extension| extension == "dart")
            && filter.admits(&path.to_string_lossy())
    }
}

fn collect_file(
    path: &Path,
    filter: &PathFilter,
    collect_manifests: bool,
    seen: &mut HashSet<PathBuf>,
    output: &mut ProjectDiscovery,
) {
    let is_manifest = collect_manifests
        && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml");
    if !admitted_file(path, filter, collect_manifests) || seen.contains(path) {
        return;
    }
    seen.insert(path.to_path_buf());
    let results = if is_manifest {
        &mut output.manifests
    } else {
        &mut output.files
    };
    results.push(path.to_path_buf());
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

    #[test]
    fn missing_input_roots_are_watched_once() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing");
        let filter = PathFilter::from_includes(&["**".into()]);
        let discovery =
            discover_project_with_watches(&[missing.clone(), missing.clone()], &filter, true);
        assert_eq!(
            discovery.watches,
            vec![DiscoveryWatch {
                path: missing.clone(),
                modified: None,
                entries: None,
            }]
        );
        assert_eq!(discovery.missing_paths, vec![missing.clone(), missing]);
    }

    #[test]
    fn absent_ancestor_manifests_are_watched() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("package/lib");
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("main.dart");
        std::fs::write(&source, "void main() {}").unwrap();
        let filter = PathFilter::from_includes(&["**".into()]);
        let discovery = discover_project_with_watches(&[source], &filter, true);
        for ancestor in root.ancestors() {
            let manifest = ancestor.join("pubspec.yaml");
            if manifest.starts_with(directory.path()) {
                assert!(discovery.watches.contains(&DiscoveryWatch {
                    path: manifest,
                    modified: None,
                    entries: None,
                }));
            }
        }
        assert!(
            !discovery
                .manifests
                .iter()
                .any(|path| path.starts_with(directory.path()))
        );
    }

    #[test]
    fn excluded_directories_are_not_watched() {
        let directory = tempfile::tempdir().unwrap();
        let excluded = directory.path().join("build");
        std::fs::create_dir_all(excluded.join("nested")).unwrap();
        let included = directory.path().join("lib");
        std::fs::create_dir(&included).unwrap();
        let mut filter = PathFilter::from_includes(&["**".into(), "!build".into()]);
        filter.set_base_dir(directory.path());
        let discovery =
            discover_project_with_watches(&[directory.path().to_path_buf()], &filter, false);
        assert!(discovery.watches.iter().any(|watch| watch.path == included));
        assert!(
            !discovery
                .watches
                .iter()
                .any(|watch| watch.path.starts_with(&excluded))
        );
    }

    #[test]
    fn directory_membership_tracks_only_admitted_names_and_types() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("lib")).unwrap();
        std::fs::create_dir(directory.path().join("excluded")).unwrap();
        std::fs::write(directory.path().join("main.dart"), "void main() {}").unwrap();
        std::fs::write(directory.path().join("ignored.dart"), "").unwrap();
        std::fs::write(directory.path().join("pubspec.yaml"), "name: app").unwrap();
        std::fs::write(directory.path().join("index.lock"), "").unwrap();
        let mut filter =
            PathFilter::from_includes(&["**".into(), "!excluded".into(), "!ignored.dart".into()]);
        filter.set_base_dir(directory.path());
        let mut discovery =
            discover_project_with_watches(&[directory.path().to_path_buf()], &filter, true);
        let watch = discovery
            .watches
            .iter_mut()
            .find(|watch| watch.path == directory.path())
            .unwrap();
        assert_eq!(
            watch.entries.as_ref().unwrap(),
            &vec![
                (OsString::from("lib"), true, false),
                (OsString::from("main.dart"), false, false),
                (OsString::from("pubspec.yaml"), false, false),
            ]
        );
        std::fs::rename(
            directory.path().join("index.lock"),
            directory.path().join("index"),
        )
        .unwrap();
        watch.modified = None;
        assert!(watch.refresh(&filter, true));
        assert_eq!(
            watch.modified,
            directory.path().metadata().unwrap().modified().ok()
        );
        std::fs::remove_file(directory.path().join("main.dart")).unwrap();
        std::fs::create_dir(directory.path().join("main.dart")).unwrap();
        watch.modified = None;
        assert!(!watch.refresh(&filter, true));
    }

    #[cfg(unix)]
    #[test]
    fn membership_retains_raw_symlink_file_names_without_following_directories() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("target.dart"), "void target() {}").unwrap();
        symlink(target.path(), directory.path().join("linked_dir")).unwrap();
        symlink(
            target.path().join("target.dart"),
            directory.path().join("alias.dart"),
        )
        .unwrap();
        let filter = PathFilter::from_includes(&["**".into()]);
        let mut discovery =
            discover_project_with_watches(&[directory.path().to_path_buf()], &filter, false);
        assert_eq!(discovery.files, vec![directory.path().join("alias.dart")]);
        let watch = discovery
            .watches
            .iter_mut()
            .find(|watch| watch.path == directory.path())
            .unwrap();
        assert_eq!(
            watch.entries.as_ref().unwrap(),
            &vec![(OsString::from("alias.dart"), false, true)]
        );
        assert!(watch.refresh(&filter, false));
        watch.modified = None;
        assert!(!watch.refresh(&filter, false));
    }

    #[test]
    fn ordinary_discovery_does_not_collect_watches() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("lib")).unwrap();
        let filter = PathFilter::from_includes(&["**".into()]);
        let discovery = discover_project(&[directory.path().to_path_buf()], &filter, true);
        assert!(discovery.watches.is_empty());
    }
}
