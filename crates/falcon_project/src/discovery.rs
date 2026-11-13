//! Discover Dart sources and package manifests in one pruning directory walk.

use falcon_analyze::{SourceAcquisition, acquire_source};
use falcon_config::PathFilter;
use rayon::prelude::*;
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
        let resolved_root = filter.resolve_walk_root(&self.path);
        for entry in directory {
            let Ok(entry) = entry else {
                return false;
            };
            let Ok(kind) = entry.file_type() else {
                return false;
            };
            let path = entry.path();
            if kind.is_dir() {
                if !filter.excludes_walked_directory(&resolved_root.join(entry.file_name())) {
                    entries.push((entry.file_name(), true, false));
                }
            } else if (kind.is_file() || (kind.is_symlink() && path.is_file()))
                && admitted_file(
                    &path,
                    Some(&resolved_root.join(entry.file_name())),
                    filter,
                    collect_manifests,
                )
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
#[derive(Debug, Default)]
pub struct ProjectDiscovery {
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    /// Metadata snapshots captured before traversing watched directories.
    /// Empty unless discovery explicitly requests watches.
    pub watches: Vec<DiscoveryWatch>,
    pub missing_paths: Vec<PathBuf>,
    /// Failures that make the discovered source set incomplete.
    pub failures: Vec<DiscoveryFailure>,
    pub filtered_paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct WalkOutput {
    pub files: Vec<(PathBuf, String)>,
    pub manifests: Vec<(PathBuf, String)>,
    /// Every discovered manifest, including those whose optional source cannot be read.
    pub manifest_paths: Vec<PathBuf>,
    pub missing_paths: Vec<PathBuf>,
    /// Explicit supported files rejected by the configured path filter.
    pub filtered_paths: Vec<PathBuf>,
}

/// A required input could not be inspected or traversed.
#[derive(Debug)]
pub enum DiscoveryFailure {
    Metadata {
        path: PathBuf,
        source: std::io::Error,
    },
    Traversal {
        path: PathBuf,
        source: walkdir::Error,
    },
}

impl std::fmt::Display for DiscoveryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Metadata { path, source } => {
                write!(
                    formatter,
                    "failed to access input {}: {source}; exclude unwanted paths with a ! pattern in files.includes",
                    path.display()
                )
            }
            Self::Traversal { path, source } => {
                write!(
                    formatter,
                    "failed to walk directory {}: {source}; exclude unwanted paths with a ! pattern in files.includes",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for DiscoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Metadata { source, .. } => Some(source),
            Self::Traversal { source, .. } => Some(source),
        }
    }
}

/// Collect matching Dart sources and, when requested, package manifests.
///
/// Includes select Dart files only. Exclusions prune directories and apply to
/// manifests, including manifests discovered in input paths' ancestors.
/// Required source read failures abort collection; ordinary optional manifest reads warn and skip.
/// Discovery failures abort collection rather than hiding required sources.
/// Reads run on the shared analysis pool; results and errors retain sorted path order.
///
/// # Errors
///
/// Returns an error if discovery fails, a required source cannot be read, or the analysis pool fails.
pub fn walk_project(
    paths: &[PathBuf],
    filter: &PathFilter,
    collect_manifests: bool,
) -> Result<WalkOutput, String> {
    let discovery = discover_project(paths, filter, collect_manifests);
    if let Some(failure) = discovery.failures.first() {
        return Err(failure.to_string());
    }
    let mut output = WalkOutput {
        missing_paths: discovery.missing_paths,
        filtered_paths: discovery.filtered_paths,
        ..WalkOutput::default()
    };
    let reads = falcon_dart_parser::with_stack(|| {
        discovery
            .files
            .into_par_iter()
            .map(|path| {
                let source = acquire_source(&path);
                (path, source)
            })
            .collect::<Vec<_>>()
    })
    .map_err(|error| format!("failed to read project sources: {error}"))?;
    // Report failures in sorted path order, independently of worker completion.
    for (path, source) in reads {
        match source {
            SourceAcquisition::Acquired(source) => output.files.push((path, source)),
            SourceAcquisition::Oversized(error) | SourceAcquisition::Failed(error) => {
                return Err(format!(
                    "failed to analyze file {}: {error}",
                    path.display()
                ));
            }
        }
    }
    output.manifest_paths.clone_from(&discovery.manifests);
    for path in discovery.manifests {
        if let Some(source) = read_manifest(&path) {
            output.manifests.push((path, source));
        }
    }
    Ok(output)
}

/// Discover supported paths, pruning exclusions without reading file contents.
#[must_use]
pub fn discover_project(
    paths: &[PathBuf],
    filter: &PathFilter,
    collect_manifests: bool,
) -> ProjectDiscovery {
    discover_project_inner(paths, filter, collect_manifests, false)
}

/// Discover paths and snapshot inputs, traversed directories, and ancestor
/// manifest candidates, including missing paths, without reading file contents.
#[must_use]
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
    manifests: bool,
    track_watches: bool,
) -> ProjectDiscovery {
    let mut output = ProjectDiscovery::default();
    let mut seen = HashSet::new();
    let mut index_by_path = track_watches.then(HashMap::new);
    let mut watches = Vec::new();
    for path in paths {
        let metadata = path.metadata();
        record_watch(path, &mut index_by_path, &mut watches, || {
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
                let failure = DiscoveryFailure::Metadata {
                    path: path.clone(),
                    source: error,
                };
                record_discovery_failure(failure, filter, manifests, &mut output);
                continue;
            }
        };
        if metadata.is_file() {
            let is_dart = path.extension().and_then(|extension| extension.to_str()) == Some("dart");
            let is_manifest = manifests
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
        if manifests
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
                record_watch(&manifest, &mut index_by_path, &mut watches, || {
                    manifest
                        .metadata()
                        .ok()
                        .and_then(|metadata| metadata.modified().ok())
                });
                if manifest.is_file() {
                    collect_file(&manifest, None, filter, true, &mut seen, &mut output);
                }
            }
        }
        if metadata.is_file() {
            collect_file(path, None, filter, manifests, &mut seen, &mut output);
        } else if metadata.is_dir() {
            discover_directory(
                path,
                filter,
                manifests,
                &mut seen,
                &mut output,
                &mut index_by_path,
                &mut watches,
            );
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

fn record_discovery_failure(
    failure: DiscoveryFailure,
    filter: &PathFilter,
    collect_manifests: bool,
    output: &mut ProjectDiscovery,
) {
    let path = match &failure {
        DiscoveryFailure::Metadata { path, .. } | DiscoveryFailure::Traversal { path, .. } => path,
    };
    if filter.excludes_directory(path) {
        return;
    }
    let required = filter.may_include_descendants(path)
        || (matches!(&failure, DiscoveryFailure::Metadata { .. })
            && admitted_file(path, None, filter, collect_manifests));
    if required {
        output.failures.push(failure);
    } else {
        tracing::warn!("{failure}; skipped because no included files can be descendants");
    }
}

fn discover_directory(
    path: &Path,
    filter: &PathFilter,
    collect_manifests: bool,
    seen: &mut HashSet<PathBuf>,
    output: &mut ProjectDiscovery,
    index_by_path: &mut Option<HashMap<PathBuf, usize>>,
    watches: &mut Vec<DiscoveryWatch>,
) {
    let track_watches = index_by_path.is_some();
    let resolved_root = filter.resolve_walk_root(path);
    // Flutter plugin symlinks point into dependency caches and may cycle.
    for item in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let kind = entry.file_type();
            if !kind.is_dir() {
                if track_watches
                    && (kind.is_file() || kind.is_symlink())
                    && admitted_file(
                        entry.path(),
                        Some(&resolved_root.join(entry.path().strip_prefix(path).unwrap())),
                        filter,
                        collect_manifests,
                    )
                {
                    record_entry(entry, index_by_path.as_ref(), watches);
                }
                return true;
            }
            let relative = entry.path().strip_prefix(path).unwrap();
            if filter.excludes_walked_directory(&resolved_root.join(relative)) {
                return false;
            }
            record_entry(entry, index_by_path.as_ref(), watches);
            let index = record_watch(entry.path(), index_by_path, watches, || {
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
                let failed_path = error.path().unwrap_or(path);
                if error
                    .io_error()
                    .is_none_or(|error| error.kind() != std::io::ErrorKind::NotFound)
                {
                    let failure = DiscoveryFailure::Traversal {
                        path: failed_path.to_path_buf(),
                        source: error,
                    };
                    record_discovery_failure(failure, filter, collect_manifests, output);
                }
                continue;
            }
        };
        #[cfg(test)]
        VISITED_ENTRIES.with_borrow_mut(|visited| visited.push(entry.path().to_path_buf()));
        let is_file = if entry.file_type().is_symlink()
            && admitted_file(
                entry.path(),
                Some(&resolved_root.join(entry.path().strip_prefix(path).unwrap())),
                filter,
                collect_manifests,
            ) {
            match entry.path().metadata() {
                Ok(metadata) => metadata.is_file(),
                Err(source) => {
                    if !matches!(
                        source.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) {
                        output.failures.push(DiscoveryFailure::Metadata {
                            path: entry.path().to_path_buf(),
                            source,
                        });
                    }
                    false
                }
            }
        } else {
            entry.file_type().is_file()
        };
        if is_file {
            let relative = entry.path().strip_prefix(path).unwrap();
            collect_file(
                entry.path(),
                Some(&resolved_root.join(relative)),
                filter,
                collect_manifests,
                seen,
                output,
            );
        }
    }
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
    index_by_path: Option<&HashMap<PathBuf, usize>>,
    watches: &mut [DiscoveryWatch],
) {
    if let Some(index_by_path) = index_by_path
        && let Some(parent) = entry.path().parent()
        && let Some(&index) = index_by_path.get(parent)
        && let Some(entries) = &mut watches[index].entries
    {
        entries.push((
            entry.file_name().to_os_string(),
            entry.file_type().is_dir(),
            entry.file_type().is_symlink(),
        ));
    }
}

fn admitted_file(
    path: &Path,
    walked_path: Option<&Path>,
    filter: &PathFilter,
    collect_manifests: bool,
) -> bool {
    if collect_manifests && path.file_name().is_some_and(|name| name == "pubspec.yaml") {
        !walked_path.map_or_else(
            || filter.excludes(&path.to_string_lossy()),
            |path| filter.excludes_walked_file(path),
        )
    } else {
        path.extension()
            .is_some_and(|extension| extension == "dart")
            && walked_path.map_or_else(
                || filter.admits(&path.to_string_lossy()),
                |path| filter.admits_walked_file(path),
            )
    }
}

fn collect_file(
    path: &Path,
    walked_path: Option<&Path>,
    filter: &PathFilter,
    collect_manifests: bool,
    seen: &mut HashSet<PathBuf>,
    output: &mut ProjectDiscovery,
) {
    let is_manifest = collect_manifests
        && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml");
    if !admitted_file(path, walked_path, filter, collect_manifests) || seen.contains(path) {
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

fn read_manifest(path: &Path) -> Option<String> {
    match falcon_config::read_metadata_text(path) {
        Ok(source) => Some(source),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read optional manifest");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn explicit_metadata_failures_are_not_missing_or_excluded_inputs() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loop");
        std::os::unix::fs::symlink(&path, &path).unwrap();
        let mut filter = PathFilter::from_includes(&["**".into()]);
        filter.set_base_dir(directory.path());
        let discovery = discover_project(std::slice::from_ref(&path), &filter, false);
        assert_eq!(discovery.missing_paths, [] as [PathBuf; 0]);
        assert!(matches!(
            &discovery.failures[..],
            [DiscoveryFailure::Metadata { path: failed, source }]
                if failed == &path && source.raw_os_error() == path.metadata().unwrap_err().raw_os_error()
        ));
        let error = walk_project(std::slice::from_ref(&path), &filter, false).unwrap_err();
        assert!(error.contains(&*path.to_string_lossy()), "{error}");
        filter = PathFilter::from_includes(&["**".into(), "!loop".into()]);
        filter.set_base_dir(directory.path());
        assert!(walk_project(std::slice::from_ref(&path), &filter, false).is_ok());
        std::fs::remove_file(&path).unwrap();
        let missing = discover_project(std::slice::from_ref(&path), &filter, false);
        assert!(missing.failures.is_empty());
        assert_eq!(missing.missing_paths, [path]);
    }

    #[cfg(unix)]
    #[test]
    fn walked_source_symlink_errors_are_reported_and_recover() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loop.dart");
        std::os::unix::fs::symlink(&path, &path).unwrap();
        let mut filter = PathFilter::from_includes(&["**".into()]);
        filter.set_base_dir(directory.path());
        for discover in [discover_project, discover_project_with_watches] {
            let discovery = discover(&[directory.path().to_path_buf()], &filter, false);
            assert!(matches!(
                &discovery.failures[..],
                [DiscoveryFailure::Metadata { path: failed, source }]
                    if failed == &path && source.raw_os_error() == path.metadata().unwrap_err().raw_os_error()
            ));
        }
        let error = walk_project(&[directory.path().to_path_buf()], &filter, false).unwrap_err();
        assert!(error.contains(&*path.to_string_lossy()), "{error}");
        assert!(error.contains("! pattern in files.includes"), "{error}");
        let mut excluded = PathFilter::from_includes(&["**".into(), "!loop.dart".into()]);
        excluded.set_base_dir(directory.path());
        assert!(walk_project(&[directory.path().to_path_buf()], &excluded, false).is_ok());
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(directory.path().join("absent.dart"), &path).unwrap();
        let absent = discover_project(&[directory.path().to_path_buf()], &filter, false);
        assert!(absent.failures.is_empty());
        assert_eq!(absent.files, [] as [PathBuf; 0]);
        std::fs::remove_file(&path).unwrap();
        let non_directory = directory.path().join("plain");
        std::fs::write(&non_directory, "").unwrap();
        std::os::unix::fs::symlink(non_directory.join("absent.dart"), &path).unwrap();
        assert_eq!(
            path.metadata().unwrap_err().kind(),
            std::io::ErrorKind::NotADirectory
        );
        let absent = discover_project(&[directory.path().to_path_buf()], &filter, false);
        assert!(absent.failures.is_empty());
        assert_eq!(absent.files, [] as [PathBuf; 0]);
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "class Recovered {}").unwrap();
        let recovered = walk_project(&[directory.path().to_path_buf()], &filter, false).unwrap();
        assert_eq!(recovered.files, [(path, "class Recovered {}".into())]);
    }

    #[test]
    fn disappearing_traversal_inputs_are_absent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("disappeared");
        let filter = PathFilter::from_includes(&["**".into()]);
        let mut output = discover_project(&[], &filter, false);
        discover_directory(
            &path,
            &filter,
            false,
            &mut HashSet::new(),
            &mut output,
            &mut None,
            &mut Vec::new(),
        );
        assert!(output.failures.is_empty());
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("recovered.dart"), "class Recovered {}").unwrap();
        let recovered = discover_project(std::slice::from_ref(&path), &filter, false);
        assert!(recovered.failures.is_empty());
        assert_eq!(recovered.files, [path.join("recovered.dart")]);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_directories_fail_unless_excluded() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let hidden = directory.path().join("hidden");
        std::fs::create_dir(&hidden).unwrap();
        std::fs::write(hidden.join("required.dart"), "class Required {}").unwrap();
        std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o0)).unwrap();
        let unreadable = std::fs::read_dir(&hidden).is_err();
        let mut filter = PathFilter::from_includes(&["**".into()]);
        filter.set_base_dir(directory.path());
        let result = walk_project(&[directory.path().to_path_buf()], &filter, false);
        let source = hidden.join("required.dart");
        let metadata_unreadable = source.metadata().is_err();
        let explicit = discover_project(std::slice::from_ref(&source), &filter, false);
        let mut excluded = PathFilter::from_includes(&["**".into(), "!hidden".into()]);
        excluded.set_base_dir(directory.path());
        let skipped = walk_project(&[directory.path().to_path_buf()], &excluded, false);
        let explicit_skipped = walk_project(std::slice::from_ref(&hidden), &excluded, false);
        let excluded_source = walk_project(std::slice::from_ref(&source), &excluded, false);
        std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(skipped.unwrap().files, [] as [(PathBuf, String); 0]);
        assert_eq!(
            explicit_skipped.unwrap().files,
            [] as [(PathBuf, String); 0]
        );
        assert_eq!(excluded_source.unwrap().files, [] as [(PathBuf, String); 0]);
        if metadata_unreadable {
            assert!(matches!(
                &explicit.failures[..],
                [DiscoveryFailure::Metadata { path, source: error }]
                    if path == &source && error.kind() == std::io::ErrorKind::PermissionDenied
            ));
        } else {
            eprintln!(
                "skipped metadata permission assertion: this user bypasses directory permissions"
            );
        }
        if unreadable {
            let error = result.unwrap_err();
            assert!(error.contains(&*hidden.to_string_lossy()), "{error}");
        } else {
            eprintln!(
                "skipped traversal permission assertion: this user bypasses directory permissions"
            );
        }
        let recovered = walk_project(&[directory.path().to_path_buf()], &filter, false).unwrap();
        assert_eq!(recovered.files.len(), 1);
        assert_eq!(recovered.files[0].0, hidden.join("required.dart"));
    }
    #[test]
    fn required_source_acquisition_retains_read_outcomes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.dart");
        std::fs::write(&path, "void main() {}").unwrap();
        assert!(matches!(
            acquire_source(&path),
            SourceAcquisition::Acquired(source) if source == "void main() {}"
        ));
        let discovery = discover_project(
            &[directory.path().to_path_buf()],
            &PathFilter::from_includes(&["**".into()]),
            false,
        );
        assert_eq!(discovery.files, vec![path.clone()]);
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            acquire_source(&discovery.files[0]),
            SourceAcquisition::Failed(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        std::fs::write(&path, [0xff]).unwrap();
        assert!(matches!(
            acquire_source(&path),
            SourceAcquisition::Failed(error) if error.kind() == std::io::ErrorKind::InvalidData
        ));
        std::fs::File::create(&path)
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
        assert!(matches!(
            acquire_source(&path),
            SourceAcquisition::Oversized(error) if falcon_analyze::is_source_too_large(&error)
        ));
        assert!(matches!(
            acquire_source(directory.path()),
            SourceAcquisition::Failed(error) if error.kind() != std::io::ErrorKind::NotFound
                && !falcon_analyze::is_source_too_large(&error)
        ));
    }

    #[test]
    fn vanished_walk_entry_is_not_an_incomplete_scan() {
        let directory = tempfile::tempdir().unwrap();
        let removed = directory.path().join("removed");
        let mut output = ProjectDiscovery::default();
        discover_directory(
            &removed,
            &PathFilter::default(),
            false,
            &mut HashSet::new(),
            &mut output,
            &mut None,
            &mut Vec::new(),
        );
        assert!(output.failures.is_empty(), "{:?}", output.failures);
    }

    #[test]
    fn replaced_walk_directory_retains_required_traversal_failure() {
        let directory = tempfile::tempdir().unwrap();
        let replaced = directory.path().join("replaced");
        std::fs::write(&replaced, "").unwrap();
        let mut output = ProjectDiscovery::default();
        discover_directory(
            &replaced.join("child"),
            &PathFilter::default(),
            false,
            &mut HashSet::new(),
            &mut output,
            &mut None,
            &mut Vec::new(),
        );
        assert!(matches!(
            output.failures.as_slice(),
            [DiscoveryFailure::Traversal { source, .. }]
                if source.io_error().is_some_and(|error| error.kind() == std::io::ErrorKind::NotADirectory)
        ));
    }

    #[test]
    fn incomplete_scan_error_explains_exclusion_remedy() {
        let failure = DiscoveryFailure::Metadata {
            path: PathBuf::from("project/private"),
            source: std::io::ErrorKind::PermissionDenied.into(),
        };
        let message = failure.to_string();
        assert!(message.contains("project/private"));
        assert!(message.contains("files.includes"), "{message}");
    }

    #[test]
    fn explicit_input_does_not_read_manifests_above_its_package() {
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("package");
        std::fs::create_dir(&package).unwrap();
        std::fs::write(directory.path().join("pubspec.yaml"), [0xff]).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: package").unwrap();
        let source = package.join("main.dart");
        std::fs::write(&source, "void main() {}").unwrap();
        let output = walk_project(&[source], &PathFilter::default(), true).unwrap();
        assert_eq!(output.manifests.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn excluded_explicit_input_is_not_accessed() {
        use std::os::unix::fs::PermissionsExt;
        struct Restore(std::path::PathBuf, std::fs::Permissions);
        impl Drop for Restore {
            fn drop(&mut self) {
                std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("private");
        std::fs::create_dir(&blocked).unwrap();
        let source = blocked.join("main.dart");
        std::fs::write(&source, "void main() {}").unwrap();
        let _restore = Restore(blocked.clone(), blocked.metadata().unwrap().permissions());
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o0)).unwrap();
        if std::process::Command::new("id")
            .arg("-u")
            .output()
            .unwrap()
            .stdout
            == b"0\n"
            || std::fs::read_dir(&blocked).is_ok()
        {
            return;
        }
        let mut filter = PathFilter::from_includes(&["**".into(), "!private".into()]);
        filter.set_base_dir(directory.path());
        let output = walk_project(&[source], &filter, false).unwrap();
        assert_eq!(output.files, [] as [(PathBuf, String); 0]);
    }

    #[test]
    fn replaced_source_parent_retains_required_source_failure() {
        let directory = tempfile::tempdir().unwrap();
        let parent = directory.path().join("replaced");
        std::fs::write(&parent, "no longer a directory").unwrap();
        let source_directory = directory.path().join("replaced.dart");
        std::fs::create_dir(&source_directory).unwrap();
        for path in [parent.join("main.dart"), source_directory] {
            assert!(matches!(
                acquire_source(&path),
                SourceAcquisition::Failed(_)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_walk_entries_are_incomplete_scans() {
        use std::os::unix::fs::PermissionsExt;
        struct Restore(std::path::PathBuf, std::fs::Permissions);
        impl Drop for Restore {
            fn drop(&mut self) {
                std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("private");
        std::fs::create_dir(&blocked).unwrap();
        std::fs::write(blocked.join("main.dart"), "void main() {}").unwrap();
        let _restore = Restore(blocked.clone(), blocked.metadata().unwrap().permissions());
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o0)).unwrap();
        if std::process::Command::new("id")
            .arg("-u")
            .output()
            .unwrap()
            .stdout
            == b"0\n"
            || std::fs::read_dir(&blocked).is_ok()
        {
            return;
        }
        let paths = [directory.path().to_path_buf()];
        let output = discover_project(&paths, &PathFilter::default(), false);
        assert!(output.failures.iter().any(|failure| {
            matches!(failure, DiscoveryFailure::Traversal { source, .. }
                if source.io_error().is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied))
        }));
        assert!(walk_project(&paths, &PathFilter::default(), false).is_err());
        let alias = directory.path().join("alias.dart");
        std::os::unix::fs::symlink(blocked.join("main.dart"), &alias).unwrap();
        let mut filter = PathFilter::from_includes(&["**".into(), "!private".into()]);
        filter.set_base_dir(directory.path());
        let output = discover_project(&paths, &filter, false);
        assert!(output.failures.iter().any(|failure| {
            matches!(failure, DiscoveryFailure::Metadata { path, source }
                if *path == alias && source.kind() == std::io::ErrorKind::PermissionDenied)
        }));
        assert!(walk_project(&paths, &filter, false).is_err());
    }
    #[test]
    fn source_reads_retain_sorted_order_and_first_size_error() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("a.dart");
        let second = directory.path().join("b.dart");
        std::fs::write(&second, "second").unwrap();
        std::fs::write(&first, "first").unwrap();
        let paths = [second.clone(), first.clone()];
        let output = walk_project(&paths, &PathFilter::default(), false).unwrap();
        assert_eq!(
            output.files,
            vec![
                (first.clone(), "first".into()),
                (second.clone(), "second".into())
            ]
        );
        for path in [&second, &first] {
            std::fs::File::create(path)
                .unwrap()
                .set_len(u64::from(u32::MAX) + 1)
                .unwrap();
        }
        let error = walk_project(&paths, &PathFilter::default(), false).unwrap_err();
        assert!(error.starts_with(&format!("failed to analyze file {}:", first.display())));
    }

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
        assert_eq!(discovery.watches, [] as [DiscoveryWatch; 0]);
    }
}
