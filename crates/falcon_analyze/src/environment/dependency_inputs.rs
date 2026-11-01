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
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Yaml(Arc<serde_yaml::Value>),
    Json(Arc<serde_json::Value>),
    Readable,
    Directory(bool),
    IsDirectory(bool),
}

#[derive(Debug)]
enum AncestorFile {
    Stale,
    Captured(Option<PathBuf>),
}

#[derive(Debug, Default)]
pub(super) struct Inputs {
    values: BTreeMap<(PathBuf, Kind), DependencyInput<Value>>,
    workspace_documents: BTreeSet<PathBuf>,
    probes: BTreeSet<PathBuf>,
    manifests: BTreeMap<PathBuf, Arc<OnceLock<Arc<PackageMetadata>>>>,
    ancestor_files: BTreeMap<(PathBuf, &'static str), AncestorFile>,
}

fn acquire(path: &Path, kind: Kind) -> DependencyInput<Value> {
    let result = (|| -> Result<Option<Value>, String> {
        if kind == Kind::IsDirectory {
            return match std::fs::metadata(path) {
                Ok(metadata) => Ok(Some(Value::IsDirectory(metadata.is_dir()))),
                Err(error) if missing(&error) => Ok(None),
                Err(error) => Err(error.to_string()),
            };
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
            Kind::Directory(_) | Kind::IsDirectory => unreachable!(),
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
        let cached = self
            .dependency_inputs
            .lock()
            .unwrap()
            .values
            .get(&key)
            .cloned();
        let input = cached.unwrap_or_else(|| {
            let acquired = acquire(&path, kind);
            self.dependency_inputs
                .lock()
                .unwrap()
                .values
                .entry(key)
                .or_insert(acquired)
                .clone()
        });
        if let DependencyInput::Unknown(reason) = &input {
            self.warn_optional_metadata(&path, &path, reason);
        }
        input
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

    /// Only mutable dependency and analyzed workspace inputs may acquire watchers.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_input_watch_roots(&self) -> Vec<PathBuf> {
        self.dependency_inputs
            .lock()
            .unwrap()
            .values
            .keys()
            .filter(|(path, kind)| {
                matches!(kind, Kind::Directory(_)) && self.mutable_dependency_input(path)
            })
            .map(|(path, _)| path.clone())
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
                matches!(kind, Kind::Readable | Kind::Json | Kind::Yaml)
                    && self.mutable_dependency_input(path)
            })
            .map(|(path, _)| path.clone())
            .chain(
                inputs
                    .probes
                    .iter()
                    .filter(|path| self.mutable_dependency_input(path))
                    .cloned(),
            )
            .collect()
    }

    /// Match captured inputs; directory entries are existence facts, not source inputs.
    ///
    /// # Panics
    /// Panics if the input cache lock is poisoned.
    #[must_use]
    pub fn dependency_input_event(&self, path: &Path) -> bool {
        let inputs = self.dependency_inputs.lock().unwrap();
        inputs.values.keys().any(|(input, kind)| {
            (self.mutable_dependency_input(input) || inputs.workspace_documents.contains(input))
                && (input.starts_with(path)
                    || (matches!(kind, Kind::Directory(_))
                        && path.parent() == Some(input.as_path())))
        }) || inputs
            .probes
            .iter()
            .any(|input| self.mutable_dependency_input(input) && input.starts_with(path))
            || inputs
                .ancestor_files
                .keys()
                .any(|(root, name)| root.ancestors().any(|ancestor| ancestor.join(name) == path))
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
        let keys: Vec<_> = self
            .dependency_inputs
            .lock()
            .unwrap()
            .values
            .keys()
            .filter(|(input, kind)| {
                paths.iter().any(|path| {
                    input.starts_with(path)
                        || (matches!(kind, Kind::Directory(_))
                            && path.parent() == Some(input.as_path()))
                })
            })
            .cloned()
            .collect();
        for (input, kind) in keys {
            let fresh = acquire(&input, kind);
            if !matches!(fresh, DependencyInput::Unknown(_)) {
                self.clear_optional_metadata_warning(&input);
            }
            self.dependency_inputs
                .lock()
                .unwrap()
                .values
                .insert((input, kind), fresh);
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
        let values = self.dependency_inputs.lock().unwrap().values.clone();
        values
            .iter()
            .filter(|((path, _), _)| {
                self.mutable_dependency_input(path)
                    || self
                        .dependency_inputs
                        .lock()
                        .unwrap()
                        .workspace_documents
                        .contains(path)
            })
            .filter(|((path, kind), value)| acquire(path, *kind) != **value)
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
}
