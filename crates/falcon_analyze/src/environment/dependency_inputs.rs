//! Raw optional dependency inputs, captured lazily per environment generation.

use super::{AnalysisEnvironment, PackageMetadata, missing};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

/// Acquisition outcome; consumers decide whether an unknown input exempts a dependency.
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyInput<T> {
    Missing,
    Unknown(String),
    Present(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Yaml,
    Json,
    Readable,
    Directory(&'static str),
    IsDirectory,
    IsFile,
    Source,
    SourceSize,
    Canonical,
    CanonicalLexical,
    Files,
}

/// Raw export syntax captured from a bounded dependency file, without retaining its AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySource {
    pub byte_len: usize,
    pub exports: Vec<DependencyExport>,
}

/// Export syntax flags; consumers retain URI resolution and traversal policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyExport {
    pub uri: String,
    pub has_combinators: bool,
    pub has_configurable_uris: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Yaml(Arc<serde_yaml::Value>),
    Json(Arc<serde_json::Value>),
    Readable,
    Directory(bool),
    IsDirectory(bool),
    IsFile(bool),
    Source(Arc<DependencySource>),
    SourceSize(u64),
    Canonical(PathBuf),
    Files(Arc<[String]>),
}

#[derive(Debug)]
enum AncestorFile {
    Stale,
    Captured(Option<PathBuf>),
}

#[derive(Debug, Default)]
pub(super) struct Inputs {
    values: BTreeMap<(PathBuf, Kind), DependencyInput<Value>>,
    stale: BTreeSet<(PathBuf, Kind)>,
    filesystem_keys: BTreeSet<(PathBuf, Kind)>,
    evicted_parents: BTreeSet<PathBuf>,
    existence_queries: bool,
    existence_overflow: bool,
    source_stamps: BTreeMap<PathBuf, Option<SourceStamp>>,
    failed: BTreeMap<PathBuf, BTreeSet<Kind>>,
    workspace_documents: BTreeSet<PathBuf>,
    probes: BTreeSet<PathBuf>,
    manifests: BTreeMap<PathBuf, Arc<OnceLock<Arc<PackageMetadata>>>>,
    ancestor_files: BTreeMap<(PathBuf, &'static str), AncestorFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceStamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    permissions: std::fs::Permissions,
    #[cfg(unix)]
    changed: (i64, i64),
}

fn source_stamp(path: &Path) -> Option<SourceStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(SourceStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        permissions: metadata.permissions(),
        #[cfg(unix)]
        changed: {
            use std::os::unix::fs::MetadataExt;
            (metadata.ctime(), metadata.ctime_nsec())
        },
    })
}

fn event_matches(input: &Path, kind: Kind, value: &DependencyInput<Value>, path: &Path) -> bool {
    let canonical = crate::canonical_package_path(input);
    let stored = match value {
        DependencyInput::Present(Value::Canonical(path)) => Some(path.as_path()),
        _ => None,
    };
    [Some(input), Some(canonical.as_path()), stored]
        .into_iter()
        .flatten()
        .any(|input| {
            input.starts_with(path)
                || (matches!(kind, Kind::Directory(_) | Kind::Files)
                    && path.parent() == Some(input))
        })
}

fn acquire_files(path: &Path, max_entries: usize) -> Result<Option<Value>, String> {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if missing(&error) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut names = Vec::new();
    for (index, child) in entries.enumerate() {
        if index >= max_entries {
            return Err("directory exceeds 100000 entries".to_owned());
        }
        let child = match child {
            Ok(child) => child,
            Err(error) if missing(&error) => continue,
            Err(error) => return Err(error.to_string()),
        };
        let metadata = match std::fs::metadata(child.path()) {
            Ok(metadata) => metadata,
            Err(error) if missing(&error) => continue,
            Err(error) => return Err(format!("{}: {error}", child.path().display())),
        };
        if metadata.is_file() {
            names.push(child.file_name().into_string().map_err(|_| {
                format!("{}: file name is not valid UTF-8", child.path().display())
            })?);
        }
    }
    names.sort_unstable();
    Ok(Some(Value::Files(names.into())))
}

fn acquire_source(source: String) -> Option<Value> {
    if source.len() > 8 * 1024 * 1024 {
        return None;
    }
    let parsed = crate::ParsedFile::parse(source);
    if parsed.has_parse_errors() {
        return None;
    }
    Some(Value::Source(Arc::new(DependencySource {
        byte_len: parsed.source().len(),
        exports: parsed
            .program()
            .exports()
            .map(|export| DependencyExport {
                uri: export.uri().value().into_owned(),
                has_combinators: export.combinators().next().is_some(),
                has_configurable_uris: export.configurable_uris().next().is_some(),
            })
            .collect(),
    })))
}

fn acquire(path: &Path, kind: Kind) -> DependencyInput<Value> {
    let result = (|| -> Result<Option<Value>, String> {
        if matches!(kind, Kind::Canonical | Kind::CanonicalLexical) {
            return Ok(Some(Value::Canonical(if kind == Kind::Canonical {
                crate::canonical_package_path(path)
            } else {
                crate::canonical_or_lexical(path)
            })));
        }
        if matches!(kind, Kind::IsDirectory | Kind::IsFile | Kind::SourceSize) {
            return match std::fs::metadata(path) {
                Ok(metadata) => Ok(Some(if kind == Kind::IsFile {
                    Value::IsFile(metadata.is_file())
                } else if kind == Kind::SourceSize {
                    Value::SourceSize(metadata.len())
                } else {
                    Value::IsDirectory(metadata.is_dir())
                })),
                Err(error) if missing(&error) => Ok(None),
                Err(error) => Err(error.to_string()),
            };
        }
        if kind == Kind::Files {
            return acquire_files(path, 100_000);
        }
        if let Kind::Directory(extension) = kind {
            let entries = match std::fs::read_dir(path) {
                Ok(entries) => entries,
                Err(error) if missing(&error) => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            for entry in entries {
                let entry = entry.map_err(|error| error.to_string())?;
                if entry
                    .path()
                    .extension()
                    .is_none_or(|value| value != extension)
                {
                    continue;
                }
                match std::fs::metadata(entry.path()) {
                    Ok(metadata) if metadata.is_file() => return Ok(Some(Value::Directory(true))),
                    Ok(_) => {}
                    Err(error) if missing(&error) => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            return Ok(Some(Value::Directory(false)));
        }
        match std::fs::metadata(path) {
            Ok(metadata) if !metadata.is_file() => return Ok(None),
            Ok(metadata) if kind == Kind::Source && metadata.len() > 8 * 1024 * 1024 => {
                return Ok(None);
            }
            Ok(_) => {}
            Err(error) if missing(&error) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        }
        let source = match falcon_config::read_metadata_text(path) {
            Ok(source) => source,
            Err(error) if missing(&error) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let text = falcon_config::metadata_content(&source);
        Ok(Some(match kind {
            Kind::Yaml => Value::Yaml(Arc::new(
                serde_yaml::from_str(text).map_err(|error| error.to_string())?,
            )),
            Kind::Json => Value::Json(Arc::new(
                serde_json::from_str(text).map_err(|error| error.to_string())?,
            )),
            Kind::Readable => Value::Readable,
            Kind::Source => return Ok(acquire_source(source)),
            Kind::Directory(_)
            | Kind::IsDirectory
            | Kind::IsFile
            | Kind::SourceSize
            | Kind::Canonical
            | Kind::CanonicalLexical
            | Kind::Files => unreachable!(),
        }))
    })();
    match result {
        Ok(Some(value)) => DependencyInput::Present(value),
        Ok(None) => DependencyInput::Missing,
        Err(reason) => DependencyInput::Unknown(reason),
    }
}

impl AnalysisEnvironment {
    /// Capture the nearest existing ancestor file, skipping non-files as discovery does.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_ancestor_file(&self, root: &Path, name: &'static str) -> Option<PathBuf> {
        let key = (root.to_path_buf(), name);
        if let Some(AncestorFile::Captured(path)) = self
            .dependency_inputs
            .lock()
            .unwrap()
            .ancestor_files
            .get(&key)
        {
            return path.clone();
        }
        let found = root
            .ancestors()
            .map(|ancestor| ancestor.join(name))
            .find(|path| path.is_file());
        self.dependency_inputs
            .lock()
            .unwrap()
            .ancestor_files
            .insert(key, AncestorFile::Captured(found.clone()));
        found
    }
    fn dependency_input(&self, path: &Path, kind: Kind) -> DependencyInput<Value> {
        let path = crate::lexical_normalize(path);
        let key = (path.clone(), kind);
        let cached = {
            let inputs = self.dependency_inputs.lock().unwrap();
            (!inputs.stale.contains(&key))
                .then(|| inputs.values.get(&key).cloned())
                .flatten()
        };
        let input = cached.unwrap_or_else(|| {
            let acquired = acquire(&path, kind);
            let mut inputs = self.dependency_inputs.lock().unwrap();
            if matches!(
                kind,
                Kind::IsFile | Kind::Canonical | Kind::CanonicalLexical
            ) && !inputs.values.contains_key(&key)
            {
                if inputs.filesystem_keys.len() >= 256
                    && let Some(old) = inputs.filesystem_keys.pop_first()
                {
                    if old.1 == Kind::IsFile
                        && self.watchable_input(&old.0, old.1)
                        && let Some(parent) = old.0.parent()
                    {
                        if inputs.evicted_parents.len() < 256 {
                            inputs.evicted_parents.insert(parent.to_path_buf());
                        } else if !inputs.evicted_parents.contains(parent) {
                            inputs.existence_overflow = true;
                        }
                    }
                    inputs.values.remove(&old);
                    inputs.stale.remove(&old);
                }
                inputs.filesystem_keys.insert(key.clone());
            }
            if kind == Kind::Source {
                inputs
                    .source_stamps
                    .insert(path.clone(), source_stamp(&path));
            }
            inputs.existence_queries |= kind == Kind::IsFile;
            if inputs.stale.remove(&key) {
                inputs.values.remove(&key);
            }
            inputs.values.entry(key).or_insert(acquired).clone()
        });
        let mut inputs = self.dependency_inputs.lock().unwrap();
        if let DependencyInput::Unknown(reason) = &input {
            inputs.failed.entry(path.clone()).or_default().insert(kind);
            drop(inputs);
            self.warn_optional_metadata(&path, &path, reason);
        } else {
            if let Some(failed) = inputs.failed.get_mut(&path) {
                failed.remove(&kind);
                if failed.is_empty() {
                    inputs.failed.remove(&path);
                }
            }
            let known = !inputs.failed.contains_key(&path);
            drop(inputs);
            if known {
                self.clear_optional_metadata_warning(&path);
            }
        }
        input
    }

    /// Memoized disk existence; unreadable targets remain unknown.
    #[must_use]
    pub fn file_exists(&self, path: &Path) -> Option<bool> {
        match self.dependency_input(&self.canonical_path(path), Kind::IsFile) {
            DependencyInput::Present(Value::IsFile(value)) => Some(value),
            DependencyInput::Missing => Some(false),
            DependencyInput::Unknown(_) => None,
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Bounded source bytes captured once per environment generation.
    #[must_use]
    pub fn dependency_source(
        &self,
        path: &Path,
        max_bytes: usize,
    ) -> DependencyInput<Arc<DependencySource>> {
        match self.dependency_input(path, Kind::SourceSize) {
            DependencyInput::Present(Value::SourceSize(bytes)) if bytes > max_bytes as u64 => {
                return DependencyInput::Missing;
            }
            DependencyInput::Unknown(reason) => return DependencyInput::Unknown(reason),
            DependencyInput::Missing | DependencyInput::Present(Value::SourceSize(_)) => {}
            DependencyInput::Present(_) => unreachable!(),
        }
        match self.dependency_input(path, Kind::Source) {
            DependencyInput::Present(Value::Source(value)) => DependencyInput::Present(value),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Capture canonical package-path comparison facts for this generation.
    #[must_use]
    pub fn canonical_path(&self, path: &Path) -> PathBuf {
        match self.dependency_input(path, Kind::Canonical) {
            DependencyInput::Present(Value::Canonical(value)) => value,
            _ => unreachable!(),
        }
    }

    /// Capture existing canonical paths, preserving lexical paths for virtual sources.
    #[must_use]
    pub fn canonical_or_lexical_path(&self, path: &Path) -> PathBuf {
        if let Some(path) = self.normalized_path(path) {
            return path.to_path_buf();
        }
        match self.dependency_input(path, Kind::CanonicalLexical) {
            DependencyInput::Present(Value::Canonical(value)) => value,
            _ => unreachable!(),
        }
    }

    /// Bounded regular-file names immediately inside a directory; dangling entries are absent.
    #[must_use]
    pub fn directory_files(&self, path: &Path) -> DependencyInput<Arc<[String]>> {
        match self.dependency_input(path, Kind::Files) {
            DependencyInput::Present(Value::Files(names)) => DependencyInput::Present(names),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Bounded parsed YAML, with no interpretation of options or include policy.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_yaml(&self, path: &Path) -> DependencyInput<Arc<serde_yaml::Value>> {
        self.dependency_inputs
            .lock()
            .unwrap()
            .workspace_documents
            .insert(crate::lexical_normalize(path));
        match self.dependency_input(path, Kind::Yaml) {
            DependencyInput::Present(Value::Yaml(value)) => DependencyInput::Present(value),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Bounded parsed JSON, with no plugin-registration policy.
    #[must_use]
    pub fn dependency_json(&self, path: &Path) -> DependencyInput<Arc<serde_json::Value>> {
        match self.dependency_input(path, Kind::Json) {
            DependencyInput::Present(Value::Json(value)) => DependencyInput::Present(value),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Whether bounded text acquisition succeeds; contents have no consumer meaning.
    #[must_use]
    pub fn dependency_readable(&self, path: &Path) -> DependencyInput<()> {
        match self.dependency_input(path, Kind::Readable) {
            DependencyInput::Present(_) => DependencyInput::Present(()),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
        }
    }

    /// Whether an immediate file has this extension; streams entries and stops on a match.
    #[must_use]
    pub fn dependency_directory(
        &self,
        path: &Path,
        extension: &'static str,
    ) -> DependencyInput<bool> {
        match self.dependency_input(path, Kind::Directory(extension)) {
            DependencyInput::Present(Value::Directory(found)) => DependencyInput::Present(found),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Captured directory existence and kind, without enumerating its children.
    #[must_use]
    pub fn dependency_is_directory(&self, path: &Path) -> DependencyInput<bool> {
        match self.dependency_input(path, Kind::IsDirectory) {
            DependencyInput::Present(Value::IsDirectory(value)) => DependencyInput::Present(value),
            DependencyInput::Missing => DependencyInput::Missing,
            DependencyInput::Unknown(reason) => DependencyInput::Unknown(reason),
            DependencyInput::Present(_) => unreachable!(),
        }
    }

    /// Exact resolved manifest, reusing the package discovery snapshot owner.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_manifest(&self, root: &Path) -> Arc<PackageMetadata> {
        self.dependency_inputs
            .lock()
            .unwrap()
            .probes
            .insert(root.join("pubspec.yaml"));
        let package = self.manifest_snapshot(&root.join("pubspec.yaml"));
        self.warn_manifest_snapshot(&package);
        package
    }

    pub(super) fn manifest_snapshot(&self, manifest: &Path) -> Arc<PackageMetadata> {
        if let Some(package) = self
            .directory_packages
            .values()
            .flatten()
            .find(|package| package.manifest == manifest)
        {
            return Arc::clone(package);
        }
        let slot = Arc::clone(
            self.dependency_inputs
                .lock()
                .unwrap()
                .manifests
                .entry(manifest.to_path_buf())
                .or_default(),
        );
        Arc::clone(slot.get_or_init(|| Arc::new(PackageMetadata::load(manifest))))
    }

    fn mutable_dependency_input(&self, path: &Path) -> bool {
        self.mutable_roots.iter().any(|root| path.starts_with(root))
            || self
                .probe_mutable_roots
                .lock()
                .unwrap()
                .iter()
                .any(|root| path.starts_with(root))
            || self
                .directory_packages
                .values()
                .flatten()
                .any(|package| path.starts_with(&package.root))
    }

    /// Set explicit workspace boundaries for rule disk facts, including pubspec-less files.
    pub fn set_rule_filesystem_roots(&mut self, roots: &[PathBuf]) {
        if self.rule_filesystem_roots != roots {
            self.rule_filesystem_roots = roots.to_vec();
        }
    }

    fn filesystem_fact_roots(&self) -> &[PathBuf] {
        if self.rule_filesystem_roots.is_empty() {
            &self.project_roots
        } else {
            &self.rule_filesystem_roots
        }
    }

    fn watchable_input(&self, path: &Path, kind: Kind) -> bool {
        if matches!(
            kind,
            Kind::IsFile | Kind::Source | Kind::SourceSize | Kind::Files
        ) {
            self.filesystem_fact_roots()
                .iter()
                .any(|root| path.starts_with(root))
                || (self.filesystem_fact_roots().is_empty()
                    && self
                        .directory_packages
                        .values()
                        .flatten()
                        .any(|package| path.starts_with(&package.root)))
        } else {
            self.mutable_dependency_input(path)
        }
    }

    /// Only mutable dependency and analyzed workspace inputs may acquire watchers.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_input_watch_roots(&self) -> Vec<PathBuf> {
        let inputs = self.dependency_inputs.lock().unwrap();
        inputs
            .values
            .keys()
            .filter(|(path, kind)| {
                matches!(kind, Kind::Directory(_) | Kind::Files)
                    && self.watchable_input(path, *kind)
            })
            .map(|(path, _)| path.clone())
            .chain(
                inputs
                    .existence_queries
                    .then_some(self.filesystem_fact_roots().iter())
                    .into_iter()
                    .flatten()
                    .cloned(),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    /// Exact mutable dependency input files, without watching ancestor directories.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_input_watch_files(&self) -> Vec<PathBuf> {
        let inputs = self.dependency_inputs.lock().unwrap();
        inputs
            .values
            .keys()
            .filter(|(path, kind)| {
                matches!(
                    kind,
                    Kind::Readable | Kind::Json | Kind::Yaml | Kind::Source | Kind::SourceSize
                ) && self.watchable_input(path, *kind)
            })
            .map(|(path, _)| path.clone())
            .chain(
                inputs
                    .probes
                    .iter()
                    .filter(|path| self.mutable_dependency_input(path))
                    .cloned(),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Match captured inputs; directory entries are existence facts, not source inputs.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_input_event(&self, path: &Path) -> bool {
        self.dependency_event(path, false)
    }

    /// Match content-sensitive inputs without refreshing existence-only facts.
    #[must_use]
    pub fn dependency_content_event(&self, path: &Path) -> bool {
        self.dependency_event(path, true)
    }

    fn dependency_event(&self, path: &Path, content_only: bool) -> bool {
        let inputs = self.dependency_inputs.lock().unwrap();
        (!content_only
            && (inputs
                .evicted_parents
                .iter()
                .any(|parent| path.starts_with(parent) || parent.starts_with(path))
                || (inputs.existence_overflow
                    && self
                        .filesystem_fact_roots()
                        .iter()
                        .any(|root| path.starts_with(root)))))
            || inputs.values.iter().any(|((input, kind), value)| {
                (!content_only
                    || !matches!(
                        kind,
                        Kind::Files | Kind::IsFile | Kind::Canonical | Kind::CanonicalLexical
                    ))
                    && (self.watchable_input(input, *kind)
                        || inputs.workspace_documents.contains(input))
                    && event_matches(input, *kind, value, path)
            })
            || inputs
                .probes
                .iter()
                .any(|input| self.mutable_dependency_input(input) && input.starts_with(path))
            || inputs
                .ancestor_files
                .keys()
                .any(|(root, name)| root.ancestors().any(|ancestor| ancestor.join(name) == path))
    }

    /// Whether a disk event changes a captured per-file rule existence query.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn file_existence_event(&self, path: &Path) -> bool {
        let inputs = self.dependency_inputs.lock().unwrap();
        (inputs
            .evicted_parents
            .iter()
            .any(|parent| path.starts_with(parent) || parent.starts_with(path))
            || (inputs.existence_overflow
                && self
                    .filesystem_fact_roots()
                    .iter()
                    .any(|root| path.starts_with(root))))
            || inputs.values.iter().any(|((input, kind), value)| {
                *kind == Kind::IsFile
                    && self.watchable_input(input, *kind)
                    && event_matches(input, *kind, value, path)
            })
    }

    /// Invalidate only acquired raw inputs on disk events; semantic indexes are unrelated.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    pub fn refresh_dependency_inputs(&self, paths: &[PathBuf]) {
        for value in self
            .dependency_inputs
            .lock()
            .unwrap()
            .ancestor_files
            .values_mut()
        {
            *value = AncestorFile::Stale;
        }
        let mut inputs = self.dependency_inputs.lock().unwrap();
        let keys: Vec<_> = inputs
            .values
            .iter()
            .filter(|((input, kind), value)| {
                paths.iter().any(|path| {
                    event_matches(input, *kind, value, path)
                        && !(*kind == Kind::Source
                            && path != input
                            && inputs
                                .source_stamps
                                .get(input)
                                .is_some_and(|stamp| source_stamp(input) == *stamp))
                })
            })
            .map(|(key, _)| key.clone())
            .collect();
        inputs.stale.extend(keys.iter().cloned());
        drop(inputs);
        for (path, kind) in keys {
            if !matches!(
                kind,
                Kind::IsFile
                    | Kind::Source
                    | Kind::Canonical
                    | Kind::CanonicalLexical
                    | Kind::Files
            ) {
                let _ = self.dependency_input(&path, kind);
            }
        }
        let manifests: Vec<_> = self
            .dependency_inputs
            .lock()
            .unwrap()
            .manifests
            .keys()
            .filter(|input| paths.iter().any(|path| input.starts_with(path)))
            .cloned()
            .collect();
        for input in manifests {
            let fresh = Arc::new(PackageMetadata::load(&input));
            if matches!(
                fresh.snapshot(),
                super::packages::ManifestSnapshot::Valid { .. }
                    | super::packages::ManifestSnapshot::Missing
            ) {
                self.clear_optional_metadata_warning(&input);
            }
            let slot = OnceLock::from(fresh);
            self.dependency_inputs
                .lock()
                .unwrap()
                .manifests
                .insert(input, Arc::new(slot));
        }
    }

    /// Fallback polling compares only facts consumed by enabled rule probes.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_input_changes(&self) -> Vec<PathBuf> {
        let inputs = self.dependency_inputs.lock().unwrap();
        let values = inputs.values.clone();
        let source_stamps = inputs.source_stamps.clone();
        let workspace_documents = inputs.workspace_documents.clone();
        drop(inputs);
        values
            .iter()
            .filter(|((path, kind), _)| {
                self.watchable_input(path, *kind) || workspace_documents.contains(path)
            })
            .filter(|((path, kind), value)| {
                if *kind == Kind::Source {
                    source_stamps
                        .get(path)
                        .is_none_or(|stamp| source_stamp(path) != *stamp)
                } else {
                    acquire(path, *kind) != **value
                }
            })
            .map(|((path, _), _)| path.clone())
            .collect()
    }

    /// Release rule-only inputs when configuration disables their consumers.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    pub fn clear_dependency_inputs(&self) {
        let mut inputs = self.dependency_inputs.lock().unwrap();
        inputs.values.clear();
        inputs.stale.clear();
        inputs.filesystem_keys.clear();
        inputs.evicted_parents.clear();
        inputs.failed.clear();
        inputs.source_stamps.clear();
        inputs.existence_queries = false;
        inputs.existence_overflow = false;
        inputs.probes.clear();
        inputs.workspace_documents.clear();
        inputs.ancestor_files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_probe_raw_facts_survive_deletion_and_share_manifest_identity() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::create_dir(root.join("bin")).unwrap();
        for (name, text) in [
            ("pubspec.yaml", "name: dep\n"),
            ("options.yaml", "include: package:lint/options.yaml\n"),
            ("plugins.json", "{}"),
            ("build.yaml", "builders: {}"),
            ("bin/tool.dart", "void main() {}"),
        ] {
            std::fs::write(root.join(name), text).unwrap();
        }
        let environment = AnalysisEnvironment::default();
        let manifest = environment.dependency_manifest(root);
        let package = environment.file_package(&root.join("lib/api.dart"));
        assert!(std::ptr::eq(manifest.as_ref(), package.metadata().unwrap()));
        let yaml = environment.dependency_yaml(&root.join("options.yaml"));
        let json = environment.dependency_json(&root.join("plugins.json"));
        let bin = environment.dependency_directory(&root.join("bin"), "dart");
        assert_eq!(
            environment.dependency_readable(&root.join("build.yaml")),
            DependencyInput::Present(())
        );
        for name in [
            "pubspec.yaml",
            "options.yaml",
            "plugins.json",
            "build.yaml",
            "bin/tool.dart",
        ] {
            std::fs::remove_file(root.join(name)).unwrap();
        }
        assert!(Arc::ptr_eq(
            &manifest,
            &environment.dependency_manifest(root)
        ));
        assert_eq!(
            environment.dependency_yaml(&root.join("options.yaml")),
            yaml
        );
        assert_eq!(
            environment.dependency_json(&root.join("plugins.json")),
            json
        );
        assert_eq!(
            environment.dependency_directory(&root.join("bin"), "dart"),
            bin
        );
        assert_eq!(
            environment.dependency_readable(&root.join("build.yaml")),
            DependencyInput::Present(())
        );
    }

    #[test]
    fn dependency_probe_missing_non_directory_and_warning_episodes() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("not_directory");
        std::fs::write(&file, "").unwrap();
        let environment = AnalysisEnvironment::default();
        let missing = file.join("options.yaml");
        assert_eq!(
            environment.dependency_yaml(&missing),
            DependencyInput::Missing
        );
        assert_eq!(
            environment.dependency_directory(&file, "dart"),
            DependencyInput::Missing
        );
        assert!(matches!(
            environment.dependency_manifest(&file).snapshot(),
            crate::ManifestSnapshot::Missing
        ));
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
        let options = root.path().join("options.yaml");
        std::fs::write(&options, "[").unwrap();
        assert!(matches!(
            environment.dependency_yaml(&options),
            DependencyInput::Unknown(_)
        ));
        assert_eq!(environment.take_warnings().len(), 1);
        environment.refresh_dependency_inputs(std::slice::from_ref(&options));
        let _ = environment.dependency_yaml(&options);
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
        std::fs::write(&options, "{}").unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&options));
        std::fs::write(&options, "[").unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&options));
        let _ = environment.dependency_yaml(&options);
        assert_eq!(environment.take_warnings().len(), 1);
    }
    #[test]
    fn dependency_manifest_warning_rearms_after_recovery() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pubspec.yaml");
        let environment = AnalysisEnvironment::default();
        for (text, warnings) in [("[", 1), ("name: dep\n", 0), ("[", 1)] {
            std::fs::write(&path, text).unwrap();
            environment.refresh_dependency_inputs(std::slice::from_ref(&path));
            let _ = environment.dependency_manifest(root.path());
            assert_eq!(environment.take_warnings().len(), warnings);
        }
    }

    #[test]
    fn dependency_documents_reject_invalid_utf8_and_oversize() {
        let root = tempfile::tempdir().unwrap();
        for bytes in [
            vec![0xff],
            vec![b' '; usize::try_from(falcon_config::MAX_METADATA_BYTES).unwrap() + 1],
        ] {
            let environment = AnalysisEnvironment::default();
            for name in [
                "analysis_options.yaml",
                ".flutter-plugins-dependencies",
                "build.yaml",
            ] {
                let path = root.path().join(name);
                std::fs::write(&path, &bytes).unwrap();
                let unknown = match name {
                    "analysis_options.yaml" => matches!(
                        environment.dependency_yaml(&path),
                        DependencyInput::Unknown(_)
                    ),
                    ".flutter-plugins-dependencies" => matches!(
                        environment.dependency_json(&path),
                        DependencyInput::Unknown(_)
                    ),
                    _ => matches!(
                        environment.dependency_readable(&path),
                        DependencyInput::Unknown(_)
                    ),
                };
                assert!(unknown, "{name}");
                assert!(
                    environment
                        .take_warnings()
                        .iter()
                        .any(|warning| warning.contains(name))
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn dependency_permission_failures_are_unknown_when_enforced() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("options.yaml");
        std::fs::write(&path, "{}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let enforced = std::fs::read(&path).is_err();
        let environment = AnalysisEnvironment::default();
        let input = environment.dependency_yaml(&path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        if !enforced {
            eprintln!("skipping permission assertion: current user bypasses mode bits");
            return;
        }
        assert!(matches!(input, DependencyInput::Unknown(_)));
        assert_eq!(environment.take_warnings().len(), 1);
    }

    #[test]
    fn dependency_directory_streams_more_than_4096_entries_without_unknown() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..4097 {
            std::fs::write(root.path().join(format!("{index}.txt")), "").unwrap();
        }
        let environment = AnalysisEnvironment::default();
        assert_eq!(
            environment.dependency_directory(root.path(), "dart"),
            DependencyInput::Present(false)
        );
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }
    #[test]
    fn dependency_ancestor_discovery_skips_non_files_and_captures_only_selection() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("app");
        std::fs::create_dir_all(root.join("analysis_options.yaml")).unwrap();
        let selected = parent.path().join("analysis_options.yaml");
        std::fs::write(&selected, "{}").unwrap();
        let environment = AnalysisEnvironment::default();
        assert_eq!(
            environment.dependency_ancestor_file(&root, "analysis_options.yaml"),
            Some(selected.clone())
        );
        assert!(matches!(
            environment.dependency_yaml(&selected),
            DependencyInput::Present(_)
        ));
        std::fs::remove_file(&selected).unwrap();
        assert_eq!(
            environment.dependency_ancestor_file(&root, "analysis_options.yaml"),
            Some(selected)
        );
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
        assert_eq!(
            environment.dependency_inputs.lock().unwrap().values.len(),
            1
        );
    }
    #[test]
    fn filesystem_fact_watchers_stay_inside_project_boundaries() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let mut environment = AnalysisEnvironment::default();
        environment.set_project_roots([project.path().to_path_buf()]);
        environment
            .mutable_roots
            .insert(outside.path().to_path_buf());
        assert_eq!(
            environment.dependency_input_watch_files(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            environment.dependency_input_watch_roots(),
            Vec::<PathBuf>::new()
        );
        for root in [project.path(), outside.path()] {
            let file = root.join("value.dart");
            std::fs::write(&file, "class Value {}").unwrap();
            assert_eq!(environment.file_exists(&file), Some(true));
            assert!(matches!(
                environment.dependency_source(&file, usize::MAX),
                DependencyInput::Present(_)
            ));
            assert!(matches!(
                environment.directory_files(root),
                DependencyInput::Present(_)
            ));
        }
        let files: BTreeSet<_> = environment
            .dependency_input_watch_files()
            .into_iter()
            .collect();
        assert_eq!(files, BTreeSet::from([project.path().join("value.dart")]));
        assert_eq!(environment.dependency_input_watch_roots(), [project.path()]);
        assert!(environment.file_existence_event(&project.path().join("value.dart")));
        assert!(!environment.file_existence_event(&outside.path().join("value.dart")));
    }

    #[test]
    fn filesystem_facts_are_memoized_until_the_affected_disk_event() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("value.dart");
        std::fs::write(&source, "class Value {}").unwrap();
        let environment = AnalysisEnvironment::default();
        assert_eq!(environment.file_exists(&source), Some(true));
        let DependencyInput::Present(parsed) = environment.dependency_source(&source, usize::MAX)
        else {
            panic!("source missing");
        };
        let canonical = environment.canonical_path(&source);
        let DependencyInput::Present(names) = environment.directory_files(root.path()) else {
            panic!("listing missing");
        };
        std::fs::remove_file(&source).unwrap();
        assert_eq!(environment.file_exists(&source), Some(true));
        let DependencyInput::Present(cached) = environment.dependency_source(&source, usize::MAX)
        else {
            panic!("source snapshot lost");
        };
        assert!(Arc::ptr_eq(&parsed, &cached));
        assert_eq!(environment.canonical_path(&source), canonical);
        assert_eq!(
            environment.directory_files(root.path()),
            DependencyInput::Present(names)
        );
        let unrelated = root.path().join("unrelated.dart");
        environment.refresh_dependency_inputs(&[unrelated]);
        assert_eq!(environment.file_exists(&source), Some(true));
        environment.refresh_dependency_inputs(std::slice::from_ref(&source));
        assert_eq!(environment.file_exists(&source), Some(false));
        assert!(matches!(
            environment.dependency_source(&source, usize::MAX),
            DependencyInput::Missing
        ));
        assert_eq!(
            environment.directory_files(root.path()),
            DependencyInput::Present(Arc::from([]))
        );
        std::fs::write(&source, "class Value {}").unwrap();
        assert_eq!(environment.file_exists(&source), Some(false));
        environment.refresh_dependency_inputs(&[root.path().to_path_buf()]);
        assert_eq!(environment.file_exists(&source), Some(true));
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }

    #[cfg(unix)]
    #[test]
    fn directory_fact_skips_dangling_links_and_rearms_only_after_recovery() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let link = root.path().join("entry");
        symlink(root.path().join("absent"), &link).unwrap();
        let environment = AnalysisEnvironment::default();
        assert_eq!(
            environment.directory_files(root.path()),
            DependencyInput::Present(Arc::from([]))
        );
        std::fs::remove_file(&link).unwrap();
        symlink(&link, &link).unwrap();
        for expected in [1, 0] {
            environment.refresh_dependency_inputs(std::slice::from_ref(&link));
            assert!(matches!(
                environment.directory_files(root.path()),
                DependencyInput::Unknown(_)
            ));
            assert_eq!(environment.take_warnings().len(), expected);
        }
        std::fs::remove_file(&link).unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&link));
        let _ = environment.directory_files(root.path());
        symlink(&link, &link).unwrap();
        environment.refresh_dependency_inputs(&[link]);
        let _ = environment.directory_files(root.path());
        assert_eq!(environment.take_warnings().len(), 1);
    }

    #[test]
    fn directory_listing_is_bounded_sorted_and_matches_child_events() {
        let root = tempfile::tempdir().unwrap();
        for name in ["z", "a", "m"] {
            std::fs::write(root.path().join(name), "").unwrap();
        }
        assert!(acquire_files(root.path(), 2).is_err());
        let mut environment = AnalysisEnvironment::default();
        environment.set_project_roots([root.path().to_path_buf()]);
        assert_eq!(
            environment.directory_files(root.path()),
            DependencyInput::Present(Arc::from(["a".to_owned(), "m".to_owned(), "z".to_owned()]))
        );
        assert!(environment.dependency_input_event(&root.path().join("new")));
        assert!(!environment.dependency_content_event(&root.path().join("new")));
    }

    #[cfg(unix)]
    #[test]
    fn existence_unknown_is_conservative_and_aliases_share_disk_facts() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file.dart");
        let alias = root.path().join("alias.dart");
        std::fs::write(&file, "").unwrap();
        symlink(&file, &alias).unwrap();
        let environment = AnalysisEnvironment::default();
        assert_eq!(environment.file_exists(&alias), Some(true));
        std::fs::remove_file(&file).unwrap();
        assert_eq!(environment.file_exists(&file), Some(true));
        environment.refresh_dependency_inputs(std::slice::from_ref(&file));
        assert_eq!(environment.file_exists(&alias), Some(false));
        let looped = root.path().join("loop.dart");
        symlink(&looped, &looped).unwrap();
        assert_eq!(environment.file_exists(&looped), None);
        assert_eq!(environment.take_warnings().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_aliases_invalidate_and_lexical_queries_keep_captured_normalization() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first.dart");
        let second = root.path().join("second.dart");
        let alias = root.path().join("alias.dart");
        std::fs::write(&first, "").unwrap();
        std::fs::write(&second, "").unwrap();
        symlink(&first, &alias).unwrap();
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([alias.as_path()], |_| None);
        assert_eq!(environment.canonical_path(&alias), first);
        std::fs::remove_file(&alias).unwrap();
        symlink(&second, &alias).unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&second));
        assert_eq!(environment.canonical_path(&alias), second);
        assert_eq!(environment.canonical_or_lexical_path(&alias), first);
    }

    #[test]
    fn filesystem_memo_and_watch_sets_are_bounded_and_sources_poll_without_reparsing() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("source.dart");
        std::fs::write(&file, "class Value {}").unwrap();
        let mut environment = AnalysisEnvironment::default();
        environment.set_project_roots([root.path().to_path_buf()]);
        let DependencyInput::Present(source) = environment.dependency_source(&file, usize::MAX)
        else {
            panic!("missing source");
        };
        assert_eq!(
            environment.dependency_input_changes(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            environment.dependency_input_changes(),
            Vec::<PathBuf>::new()
        );
        environment.refresh_dependency_inputs(&[root.path().to_path_buf()]);
        let DependencyInput::Present(cached) = environment.dependency_source(&file, usize::MAX)
        else {
            panic!("missing source");
        };
        assert!(Arc::ptr_eq(&source, &cached));
        for index in 0..600 {
            assert_eq!(
                environment.file_exists(&root.path().join(format!("{index}.txt"))),
                Some(false)
            );
        }
        assert!(environment.dependency_inputs.lock().unwrap().values.len() <= 258);
        assert_eq!(environment.dependency_input_watch_roots(), [root.path()]);
        assert!(environment.dependency_input_watch_files().len() <= 1);
        for _ in 0..2 {
            assert_eq!(
                environment.dependency_input_changes(),
                Vec::<PathBuf>::new()
            );
        }
        let evicted = root.path().join("0.txt");
        std::fs::write(&evicted, "").unwrap();
        assert!(environment.file_existence_event(&evicted));
        assert_eq!(
            environment.dependency_input_changes(),
            Vec::<PathBuf>::new()
        );
        environment.refresh_dependency_inputs(&[root.path().to_path_buf()]);
        assert_eq!(environment.file_exists(&evicted), Some(true));
    }

    #[test]
    fn filesystem_existence_events_ignore_unqueried_workspace_paths() {
        let root = tempfile::tempdir().unwrap();
        let mut environment = AnalysisEnvironment::default();
        environment.set_project_roots([root.path().to_path_buf()]);
        let target = root.path().join("test/value_test.dart");
        assert_eq!(environment.file_exists(&target), Some(false));
        let unrelated = root.path().join("build/out.txt");
        assert!(!environment.file_existence_event(&unrelated));
        assert!(!environment.dependency_input_event(&unrelated));
        assert!(environment.file_existence_event(&target));
        assert!(environment.file_existence_event(target.parent().unwrap()));
    }

    #[test]
    fn filesystem_evicted_parent_tracking_is_bounded_and_clear_resets_bookkeeping() {
        let root = tempfile::tempdir().unwrap();
        let mut environment = AnalysisEnvironment::default();
        environment.set_project_roots([root.path().to_path_buf()]);
        for index in 0..600 {
            let _ = environment.file_exists(&root.path().join(format!("test/{index}.dart")));
        }
        let unrelated = root.path().join("build/out.txt");
        assert!(!environment.file_existence_event(&unrelated));
        {
            let inputs = environment.dependency_inputs.lock().unwrap();
            assert_eq!(inputs.filesystem_keys.len(), 256);
            assert_eq!(inputs.evicted_parents.len(), 1);
            assert!(!inputs.existence_overflow);
        }
        for index in 0..600 {
            let _ = environment.file_exists(&root.path().join(format!("dir{index:03}/test.dart")));
        }
        {
            let mut inputs = environment.dependency_inputs.lock().unwrap();
            assert_eq!(inputs.evicted_parents.len(), 256);
            assert!(inputs.existence_overflow);
            inputs
                .failed
                .entry(unrelated.clone())
                .or_default()
                .insert(Kind::IsFile);
        }
        assert!(environment.file_existence_event(&unrelated));
        assert_eq!(
            environment.dependency_input_changes(),
            Vec::<PathBuf>::new()
        );
        environment.clear_dependency_inputs();
        let inputs = environment.dependency_inputs.lock().unwrap();
        assert!(inputs.filesystem_keys.is_empty());
        assert!(inputs.evicted_parents.is_empty());
        assert!(inputs.failed.is_empty());
        assert!(!inputs.existence_overflow);
    }

    #[test]
    fn source_budget_is_checked_before_reading_and_does_not_poison_larger_queries() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("source.dart");
        std::fs::write(&file, [0xff; 16]).unwrap();
        let environment = AnalysisEnvironment::default();
        assert!(matches!(
            environment.dependency_source(&file, 15),
            DependencyInput::Missing
        ));
        assert_eq!(environment.take_warnings().len(), 0);
        assert!(
            !environment
                .dependency_inputs
                .lock()
                .unwrap()
                .values
                .contains_key(&(file.clone(), Kind::Source))
        );
        assert!(matches!(
            environment.dependency_source(&file, 16),
            DependencyInput::Unknown(_)
        ));
        assert_eq!(environment.take_warnings().len(), 1);
        std::fs::remove_file(&file).unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&file));
        assert!(matches!(
            environment.dependency_source(&file, 16),
            DependencyInput::Missing
        ));
        std::fs::write(&file, [0xff; 16]).unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&file));
        assert!(matches!(
            environment.dependency_source(&file, 16),
            DependencyInput::Unknown(_)
        ));
        assert_eq!(environment.take_warnings().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn source_stamp_observes_permission_changes() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("source.dart");
        std::fs::write(&file, "class Value {}").unwrap();
        let mut environment = AnalysisEnvironment::default();
        environment.set_project_roots([root.path().to_path_buf()]);
        let _ = environment.dependency_source(&file, usize::MAX);
        let before = source_stamp(&file);
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode ^ 0o100)).unwrap();
        assert_ne!(source_stamp(&file), before);
        assert_eq!(environment.dependency_input_changes(), [file]);
    }

    #[cfg(unix)]
    #[test]
    fn inaccessible_existence_warning_episode_survives_eviction_and_canonical_queries() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let looped = root.path().join("000-loop.dart");
        symlink(&looped, &looped).unwrap();
        let environment = AnalysisEnvironment::default();
        for expected in [1, 0, 0] {
            assert_eq!(environment.file_exists(&looped), None);
            assert_eq!(environment.take_warnings().len(), expected);
            for index in 0..300 {
                let _ = environment.file_exists(&root.path().join(format!("z{index}.txt")));
            }
            let _ = environment.canonical_path(&looped);
        }
        std::fs::remove_file(&looped).unwrap();
        std::fs::write(&looped, "").unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&looped));
        assert_eq!(environment.file_exists(&looped), Some(true));
        std::fs::remove_file(&looped).unwrap();
        symlink(&looped, &looped).unwrap();
        environment.refresh_dependency_inputs(std::slice::from_ref(&looped));
        assert_eq!(environment.file_exists(&looped), None);
        assert_eq!(environment.take_warnings().len(), 1);
    }
}
