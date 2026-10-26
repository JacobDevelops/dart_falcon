//! Pubspec discovery and immutable package facts shared by file rules.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{PackageIdentity, canonical_or_lexical};

/// A package manifest as observed when the analysis environment was discovered.
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    pub manifest: PathBuf,
    pub identity: PackageIdentity,
    pub(super) root: PathBuf,
    yaml: Option<serde_yaml::Value>,
}

impl PackageMetadata {
    pub fn load(manifest: &Path) -> Self {
        let source = std::fs::read_to_string(manifest).ok();
        let yaml = source
            .as_deref()
            .and_then(|source| serde_yaml::from_str::<serde_yaml::Value>(source).ok());
        let name = yaml
            .as_ref()
            .and_then(|yaml| yaml.get("name"))
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                source
                    .as_deref()
                    .filter(|_| yaml.is_none())
                    .and_then(|source| {
                        source.lines().find_map(|line| {
                            serde_yaml::from_str::<String>(line.strip_prefix("name:")?.trim()).ok()
                        })
                    })
            })
            .unwrap_or_default();
        let root = canonical_or_lexical(manifest.parent().unwrap());
        Self {
            manifest: super::package_config::absolute(manifest),
            identity: PackageIdentity {
                name,
                lib_root: canonical_lib_root(&root),
            },
            root,
            yaml,
        }
    }

    /// The strictly parsed name; malformed manifests are not valid import owners.
    pub fn parsed_name(&self) -> Option<&str> {
        self.yaml.as_ref()?.as_mapping()?.get("name")?.as_str()
    }

    /// Preserve canonical directory lookups for saved and virtual symlink aliases.
    pub(super) fn directory_alias(&self, directory: &Path) -> Option<PathBuf> {
        let canonical = canonical_or_lexical(directory);
        (canonical != directory
            && canonical.starts_with(&self.root)
            && canonical
                .ancestors()
                .map(|directory| directory.join("pubspec.yaml"))
                .find(|manifest| manifest.is_file())
                .is_some_and(|manifest| manifest.parent() == Some(self.root.as_path())))
        .then_some(canonical)
    }

    pub fn checks_flutter_web_imports(&self) -> bool {
        self.yaml.as_ref().is_some_and(|yaml| {
            !yaml["dependencies"]["flutter"].is_null()
                && yaml["flutter"]["plugin"]["platforms"]["web"].is_null()
        })
    }
}

/// Package identity and normalized location computed once for an analyzed file.
#[derive(Debug, Clone)]
pub struct FilePackage {
    package: Option<Arc<PackageMetadata>>,
    in_lib: bool,
    directory: Option<String>,
}

impl FilePackage {
    pub fn discover(path: &Path) -> Self {
        let path = canonical_or_lexical(&super::package_config::absolute(path));
        let package = enclosing_pubspec_normalized(&path)
            .map(|manifest| Arc::new(PackageMetadata::load(&manifest)));
        Self::new(&path, package)
    }

    pub(super) fn new(path: &Path, package: Option<Arc<PackageMetadata>>) -> Self {
        if package.is_none() {
            return Self {
                package: None,
                in_lib: false,
                directory: None,
            };
        }
        let lexical = path.to_path_buf();
        let directory = package.as_ref().and_then(|package| {
            lexical
                .strip_prefix(package.manifest.parent()?)
                .or_else(|_| lexical.strip_prefix(&package.root))
                .ok()?
                .components()
                .next()?
                .as_os_str()
                .to_str()
                .map(str::to_owned)
        });
        let canonical_path = if let Some(package) = &package {
            let root = package.manifest.parent().unwrap();
            match lexical
                .strip_prefix(root)
                .or_else(|_| lexical.strip_prefix(&package.root))
            {
                Ok(relative) => {
                    if let Ok(relative) = relative.strip_prefix("lib") {
                        package.identity.lib_root.join(relative)
                    } else {
                        package.root.join(relative)
                    }
                }
                Err(_) => lexical,
            }
        } else {
            lexical
        };
        let in_lib = package
            .as_ref()
            .is_some_and(|package| canonical_path.starts_with(&package.identity.lib_root));
        Self {
            package,
            in_lib,
            directory,
        }
    }

    pub fn metadata(&self) -> Option<&PackageMetadata> {
        self.package.as_deref()
    }

    pub fn identity(&self) -> Option<&PackageIdentity> {
        self.metadata().map(|package| &package.identity)
    }

    pub fn lib_identity(&self) -> Option<&PackageIdentity> {
        self.identity().filter(|_| self.in_lib)
    }

    pub fn in_directory(&self, name: &str) -> bool {
        self.directory.as_deref() == Some(name)
    }
}

/// Find the nearest manifest without resolving symlinks inside virtual sources.
pub fn enclosing_pubspec(path: &Path) -> Option<PathBuf> {
    let path = canonical_or_lexical(path);
    enclosing_pubspec_normalized(&path)
}

pub(super) fn enclosing_pubspec_normalized(path: &Path) -> Option<PathBuf> {
    if path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml") && path.is_file() {
        return Some(path.to_path_buf());
    }
    path.parent()?
        .ancestors()
        .map(|directory| directory.join("pubspec.yaml"))
        .find(|manifest| manifest.is_file())
}

/// Resolve existing prefixes so saved and missing paths share one comparison form.
pub(crate) fn canonical_comparison_path(path: &Path) -> PathBuf {
    let path = super::package_config::absolute(path);
    for ancestor in path.ancestors() {
        if let Ok(canonical) = std::fs::canonicalize(ancestor) {
            return canonical.join(path.strip_prefix(ancestor).unwrap());
        }
    }
    path
}

pub fn canonical_package_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        let path = canonical_or_lexical(path);
        let Some(manifest) = enclosing_pubspec(&path) else {
            return canonical_comparison_path(&path);
        };
        let root = manifest.parent().unwrap();
        let Ok(relative) = path.strip_prefix(root) else {
            return path;
        };
        if let Ok(relative) = relative.strip_prefix("lib") {
            canonical_lib_root(root).join(relative)
        } else {
            canonical_or_lexical(root).join(relative)
        }
    })
}

fn canonical_lib_root(root: &Path) -> PathBuf {
    std::fs::canonicalize(root.join("lib"))
        .unwrap_or_else(|_| canonical_or_lexical(root).join("lib"))
}

pub fn package_identity(manifest: &Path) -> PackageIdentity {
    PackageMetadata::load(manifest).identity
}

/// Collect all enclosing packages, including outer packages of nested projects.
pub fn package_identities<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Vec<PackageIdentity> {
    let mut directories: Vec<_> = paths
        .into_iter()
        .filter_map(Path::parent)
        .flat_map(Path::ancestors)
        .collect();
    directories.sort_unstable();
    directories.dedup();
    let mut packages: Vec<_> = directories
        .into_iter()
        .map(|directory| directory.join("pubspec.yaml"))
        .filter(|manifest| manifest.is_file())
        .map(|manifest| package_identity(&manifest))
        .collect();
    packages.sort_unstable_by(|left, right| left.lib_root.cmp(&right.lib_root));
    packages.dedup_by(|left, right| left.lib_root == right.lib_root);
    packages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisEnvironment, AnalyzeContext, ParsedFile};
    use falcon_config::FalconConfig;

    fn parsed() -> Arc<ParsedFile> {
        Arc::new(ParsedFile {
            source: "".into(),
            tree: falcon_dart_parser::parse("").tree,
        })
    }

    #[test]
    fn context_shares_package_facts_and_rebinds_for_another_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("lib")).unwrap();
        let manifest = root.path().join("pubspec.yaml");
        std::fs::write(&manifest, "name: first\n").unwrap();
        let source = root.path().join("lib/source.dart");
        let test = root.path().join("test/source.dart");
        let config = FalconConfig::default();
        let parsed = parsed();
        let ctx = AnalyzeContext::new(&source, &config, &parsed);
        assert_eq!(ctx.package().unwrap().name, "first");
        assert!(std::ptr::eq(
            ctx.package().unwrap(),
            ctx.lib_dir_package().unwrap()
        ));
        std::fs::write(&manifest, "name: changed\n").unwrap();
        assert_eq!(ctx.package().unwrap().name, "first");
        let rebound = ctx.for_file(&test, &parsed);
        assert_eq!(rebound.package().unwrap().name, "changed");
        assert!(rebound.file_package().in_directory("test"));
        assert!(rebound.lib_dir_package().is_none());
    }

    #[test]
    fn environment_owns_manifest_snapshot_until_reset() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("lib")).unwrap();
        let manifest = root.path().join("pubspec.yaml");
        let source = root.path().join("lib/source.dart");
        let other = root.path().join("lib/other.dart");
        std::fs::write(&manifest, "name: first\n").unwrap();
        let mut environment = AnalysisEnvironment::discover_with_sdk_locator(
            [source.as_path(), other.as_path()],
            |_| None,
        );
        assert!(std::ptr::eq(
            environment.package(&source).unwrap(),
            environment.package(&other).unwrap(),
        ));
        std::fs::remove_file(&manifest).unwrap();
        let parsed = parsed();
        let config = FalconConfig::default();
        let ctx =
            AnalyzeContext::new(&source, &config, &parsed).with_environment(Some(&environment));
        assert_eq!(ctx.package().unwrap().name, "first");
        assert!(ctx.in_same_package(&other));
        environment.reset();
        environment.extend([source.as_path()]);
        assert!(environment.package(&source).is_none());
    }

    #[test]
    fn external_package_discovery_is_shared_across_contexts_and_directories() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        std::fs::create_dir_all(root.path().join("lib/src")).unwrap();
        std::fs::write(&manifest, "name: external\n").unwrap();
        let first = root.path().join("lib/first.dart");
        let second = root.path().join("lib/src/second.dart");
        let mut environment = AnalysisEnvironment::default();
        let first_facts = environment.file_package(&first);
        std::fs::write(&manifest, "name: changed\n").unwrap();
        let second_facts = environment.file_package(&second);
        assert!(std::ptr::eq(
            first_facts.metadata().unwrap(),
            second_facts.metadata().unwrap(),
        ));
        assert_eq!(second_facts.identity().unwrap().name, "external");
        environment.reset();
        assert_eq!(
            environment.file_package(&second).identity().unwrap().name,
            "changed"
        );
    }

    #[test]
    fn lazy_package_inputs_are_watched_after_the_initial_environment_key() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("lib/nested");
        std::fs::create_dir_all(&nested).unwrap();
        let manifest = root.path().join("pubspec.yaml");
        let nearer_manifest = nested.join("pubspec.yaml");
        std::fs::write(&manifest, "name: original\n").unwrap();
        let source = nested.join("source.dart");
        let mut environment = AnalysisEnvironment::default();
        let initial = environment.config_stamps();
        assert!(!initial.affected_by(&manifest));
        assert_eq!(
            environment.file_package(&source).identity().unwrap().name,
            "original"
        );
        assert!(environment.package_inputs_affected_by(&manifest));
        assert!(environment.package_inputs_affected_by(&nearer_manifest));
        let discovered = environment.config_stamps();
        assert!(discovered.affected_by(&manifest));
        assert!(discovered.affected_by(&nearer_manifest));
        std::fs::write(&nearer_manifest, "name: nested\n").unwrap();
        assert_ne!(discovered, environment.config_stamps());
        environment.reset();
        assert_eq!(
            environment.file_package(&source).identity().unwrap().name,
            "nested"
        );
    }

    #[test]
    fn lazy_package_admission_detects_changes_after_discovery_before_idle() {
        for nearer in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let nested = root.path().join("lib/nested");
            std::fs::create_dir_all(&nested).unwrap();
            let manifest = root.path().join("pubspec.yaml");
            std::fs::write(&manifest, "name: original\n").unwrap();
            let source = nested.join("source.dart");
            let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            let mut baseline = environment.config_stamps();
            assert_eq!(
                environment.file_package(&source).identity().unwrap().name,
                "original"
            );
            let changed = if nearer {
                nested.join("pubspec.yaml")
            } else {
                manifest
            };
            std::fs::write(&changed, "name: changed\n").unwrap();

            // The first idle sample is already newer than the loaded metadata.
            assert!(baseline.extend_discovery(environment.config_stamps()));
            assert_eq!(
                environment.file_package(&source).identity().unwrap().name,
                "original"
            );
            assert!(baseline.extend_discovery(environment.config_stamps()));
            environment.reset();
            assert_eq!(
                environment.file_package(&source).identity().unwrap().name,
                "changed"
            );
        }
    }

    #[test]
    fn unchanged_lazy_package_admission_preserves_the_first_read_baseline() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("lib")).unwrap();
        let manifest = root.path().join("pubspec.yaml");
        std::fs::write(&manifest, "name: original\n").unwrap();
        let source = root.path().join("lib/source.dart");
        let environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let mut baseline = environment.config_stamps();
        environment.file_package(&source);
        assert!(!baseline.extend_discovery(environment.config_stamps()));
        assert!(!baseline.extend_discovery(environment.config_stamps()));
        std::fs::write(&manifest, "name: changed\n").unwrap();
        assert!(baseline.extend_discovery(environment.config_stamps()));
    }

    #[test]
    fn concurrent_external_discovery_publishes_one_manifest_snapshot() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("lib/src")).unwrap();
        std::fs::write(root.path().join("pubspec.yaml"), "name: shared\n").unwrap();
        let first = root.path().join("lib/first.dart");
        let second = root.path().join("lib/src/second.dart");
        let environment = AnalysisEnvironment::default();
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| environment.file_package(&first));
            let second = scope.spawn(|| environment.file_package(&second));
            (first.join().unwrap(), second.join().unwrap())
        });
        assert!(std::ptr::eq(
            first.metadata().unwrap(),
            second.metadata().unwrap()
        ));
    }

    #[test]
    fn interleaved_package_discovery_keeps_the_first_fingerprint_baseline() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("lib")).unwrap();
        std::fs::write(root.path().join("pubspec.yaml"), "name: original\n").unwrap();
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let mut baseline = environment.config_stamps();
        environment.io_observer = Some(|environment, path| {
            if !environment.package_discovery.lock().unwrap().manifests.is_empty() {
                return;
            }
            // B has selected its uncached probes but has not read them yet.
            // Let A publish v1, then make B read v2 and reuse A's manifest v1.
            let mut first_worker = environment.clone();
            first_worker.io_observer = None;
            let first = first_worker.file_package(&path.with_file_name("first.dart"));
            let metadata = first.metadata().unwrap();
            assert_eq!(metadata.identity.name, "original");
            std::fs::write(&metadata.manifest, "name: modified\n").unwrap();
        });
        let second = environment.file_package(&root.path().join("lib/second.dart"));
        assert_eq!(second.identity().unwrap().name, "original");
        environment.io_observer = None;
        assert!(baseline.extend_discovery(environment.config_stamps()));
    }

    #[test]
    fn package_discovery_releases_the_shared_lock_before_io() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("lib/src")).unwrap();
        std::fs::write(root.path().join("pubspec.yaml"), "name: shared\n").unwrap();
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        environment.io_observer = Some(|environment, _| {
            assert!(environment.package_discovery.try_lock().is_ok());
        });
        for directory in ["lib", "lib/src"] {
            let source = root.path().join(directory).join("source.dart");
            assert_eq!(
                environment.file_package(&source).identity().unwrap().name,
                "shared"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn repeated_config_stamps_do_not_reread_unchanged_contents() {
        let root = tempfile::tempdir().unwrap();
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        environment.io_observer = Some(|environment, _| {
            assert!(environment.config_fingerprints.try_lock().is_ok());
        });
        // Simulate files whose change timestamps predate the sampling tick,
        // without sleeping or relying on the host filesystem's clock resolution.
        environment.observation_time =
            Some(std::time::SystemTime::now() + std::time::Duration::from_secs(3600));
        for index in 0..32 {
            let path = root.path().join(format!("config-{index}.json"));
            std::fs::write(&path, format!("{{\"index\": {index}}}")).unwrap();
            environment.config_candidates.insert(path);
        }
        environment
            .config_candidates
            .insert(root.path().join("missing.json"));
        let original = environment.config_stamps();
        let reads = environment.config_fingerprints.lock().unwrap().reads;
        assert!(reads >= 32);
        for _ in 0..16 {
            let current = environment.config_stamps();
            assert_eq!(original, current);
            for (previous, current) in original.files.iter().zip(&current.files) {
                if let (Some(previous), Some(current)) = (&previous.2, &current.2) {
                    assert!(Arc::ptr_eq(previous, current));
                }
            }
        }
        assert_eq!(environment.config_fingerprints.lock().unwrap().reads, reads);

        let changed = root.path().join("config-0.json");
        std::fs::write(&changed, "{\"changed\": true}").unwrap();
        assert_ne!(original, environment.config_stamps());
        assert_eq!(
            environment.config_fingerprints.lock().unwrap().reads,
            reads + 1
        );
    }

    #[cfg(unix)]
    #[test]
    fn config_stamps_reread_racy_metadata_until_a_later_observation() {
        use std::os::unix::fs::MetadataExt;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pubspec.yaml");
        std::fs::write(&path, "name: original\n").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        environment.observation_time = Some(
            std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(metadata.ctime().try_into().unwrap()),
        );
        let original = environment.config_fingerprint(&path);
        std::fs::write(&path, "name: modified\n").unwrap();
        let current_metadata = std::fs::metadata(&path).unwrap();
        // Emulate a coarse filesystem reporting unchanged metadata across the
        // rewrite: the cached bytes remain v1 while both stat samples match v2.
        environment
            .config_fingerprints
            .lock()
            .unwrap()
            .files
            .get_mut(&path)
            .unwrap()
            .metadata = Some(current_metadata.clone());
        let changed = environment.config_fingerprint(&path);
        assert_ne!(original, changed);
        assert_eq!(environment.config_fingerprints.lock().unwrap().reads, 2);
        environment.observation_time = Some(
            std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(
                    u64::try_from(current_metadata.ctime()).unwrap() + 2,
                ),
        );
        // The first later sample must read again before its bytes become stable.
        assert_eq!(changed, environment.config_fingerprint(&path));
        assert_eq!(environment.config_fingerprints.lock().unwrap().reads, 3);
        assert_eq!(changed, environment.config_fingerprint(&path));
        assert_eq!(environment.config_fingerprints.lock().unwrap().reads, 3);
    }

    #[test]
    fn config_stamps_track_creation_deletion_and_metadata_only_changes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pubspec.yaml");
        let mut environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        environment.config_candidates.insert(path.clone());
        let missing = environment.config_stamps();
        std::fs::write(&path, "name: original\n").unwrap();
        let original = environment.config_stamps();
        assert_ne!(missing, original);
        let metadata = std::fs::metadata(&path).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(
                metadata.modified().unwrap() + std::time::Duration::from_secs(1),
            ))
            .unwrap();
        assert_eq!(original, environment.config_stamps());
        std::fs::write(&path, "name: modified\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), metadata.len());
        let modified = environment.config_stamps();
        assert_ne!(original, modified);
        let replacement = root.path().join("replacement.yaml");
        std::fs::write(&replacement, "name: replaced\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&replacement)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), metadata.len());
        assert_ne!(modified, environment.config_stamps());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(missing, environment.config_stamps());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_do_not_poison_neighboring_package_metadata() {
        let root = tempfile::tempdir().unwrap();
        let host = root.path().join("host");
        let target = root.path().join("target");
        for (directory, name) in [(&host, "host"), (&target, "target")] {
            std::fs::create_dir_all(directory.join("lib")).unwrap();
            std::fs::write(directory.join("pubspec.yaml"), format!("name: {name}\n")).unwrap();
        }
        let ordinary = host.join("lib/ordinary.dart");
        let linked = host.join("lib/linked.dart");
        let original = target.join("lib/original.dart");
        std::fs::write(&ordinary, "").unwrap();
        std::fs::write(&original, "").unwrap();
        std::os::unix::fs::symlink(&original, &linked).unwrap();
        for paths in [
            [linked.as_path(), ordinary.as_path()],
            [ordinary.as_path(), linked.as_path()],
        ] {
            let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, |_| None);
            for (path, expected) in [(&ordinary, "host"), (&linked, "target")] {
                assert_eq!(environment.package(path).unwrap().identity.name, expected);
                assert_eq!(
                    environment.file_package(path).identity().unwrap().name,
                    expected
                );
                assert_eq!(
                    FilePackage::discover(path).identity().unwrap().name,
                    expected
                );
            }
            let lazy = AnalysisEnvironment::default();
            for path in paths {
                let expected = if path == linked { "target" } else { "host" };
                assert_eq!(lazy.file_package(path).identity().unwrap().name, expected);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn virtual_directory_aliases_do_not_override_nested_package_metadata() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("vendor/nested");
        std::fs::create_dir_all(root.path().join("lib")).unwrap();
        std::fs::create_dir_all(nested.join("lib")).unwrap();
        std::fs::write(root.path().join("pubspec.yaml"), "name: outer\n").unwrap();
        std::fs::write(nested.join("pubspec.yaml"), "name: nested\n").unwrap();
        let link = root.path().join("lib/link");
        std::os::unix::fs::symlink(nested.join("lib"), &link).unwrap();
        let virtual_source = link.join("unsaved.dart");
        let source = nested.join("lib/source.dart");
        std::fs::write(&source, "").unwrap();
        for paths in [
            [virtual_source.as_path(), source.as_path()],
            [source.as_path(), virtual_source.as_path()],
        ] {
            let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, |_| None);
            assert_eq!(
                environment.file_package(&source).identity().unwrap().name,
                "nested"
            );
            assert_eq!(
                environment
                    .file_package(&virtual_source)
                    .identity()
                    .unwrap()
                    .name,
                "outer"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn missing_config_inputs_match_events_through_directory_aliases() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("directory");
        let alias = root.path().join("alias");
        std::fs::create_dir(&directory).unwrap();
        std::os::unix::fs::symlink(&directory, &alias).unwrap();
        for (input, event) in [(&directory, &alias), (&alias, &directory)] {
            let mut environment = AnalysisEnvironment::default();
            environment
                .config_candidates
                .insert(input.join("missing/pubspec.yaml"));
            let stamps = environment.config_stamps();
            assert!(stamps.affected_by(&event.join("missing")));
            assert!(stamps.affected_by(&event.join("missing/pubspec.yaml")));
            assert!(!stamps.affected_by(&event.join("unrelated")));
        }
    }

    #[cfg(unix)]
    #[test]
    fn config_events_retain_the_captured_symlink_target() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let alias = root.path().join("alias");
        let bridge = root.path().join("bridge");
        let home = root.path().join("home");
        for directory in [&first, &second] {
            std::fs::create_dir(directory).unwrap();
            std::fs::write(directory.join("pubspec.yaml"), "name: dependency\n").unwrap();
        }
        std::os::unix::fs::symlink(&first, &bridge).unwrap();
        std::os::unix::fs::symlink(&bridge, &alias).unwrap();
        std::os::unix::fs::symlink(root.path(), &home).unwrap();
        let mut environment = AnalysisEnvironment::default();
        environment
            .config_candidates
            .insert(home.join("alias/pubspec.yaml"));
        let captured = environment.config_stamps();
        std::fs::remove_file(&bridge).unwrap();
        std::os::unix::fs::symlink(&second, &bridge).unwrap();
        assert!(captured.affected_by(&first.join("pubspec.yaml")));
        assert!(captured.affected_by(&home.join("alias/pubspec.yaml")));
        let canonical_parent = std::fs::canonicalize(root.path()).unwrap();
        assert!(captured.affected_by(&canonical_parent.join("bridge")));
        assert!(!captured.affected_by(&root.path().join("unrelated")));
        let refreshed = environment.config_stamps();
        assert_ne!(captured, refreshed);
        assert!(refreshed.affected_by(&second.join("pubspec.yaml")));
        assert!(!refreshed.affected_by(&first.join("pubspec.yaml")));
        std::fs::remove_file(&alias).unwrap();
        assert!(refreshed.affected_by(&canonical_parent.join("alias")));
    }

    #[cfg(unix)]
    #[test]
    fn config_events_capture_links_after_physical_parent_traversal() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let nested = root.path().join("x/y");
        let bridge = root.path().join("x/b");
        for directory in [&first, &second, &nested] {
            std::fs::create_dir_all(directory).unwrap();
        }
        for directory in [&first, &second] {
            std::fs::write(directory.join("pubspec.yaml"), "name: dependency\n").unwrap();
        }
        std::os::unix::fs::symlink("x/y", root.path().join("a")).unwrap();
        std::os::unix::fs::symlink("../b", nested.join("link")).unwrap();
        std::os::unix::fs::symlink("a/../b", root.path().join("c")).unwrap();
        let canonical_root = std::fs::canonicalize(root.path()).unwrap();
        for input in ["a/link/pubspec.yaml", "c/pubspec.yaml"] {
            std::os::unix::fs::symlink(&first, &bridge).unwrap();
            let mut environment = AnalysisEnvironment::default();
            environment
                .config_candidates
                .insert(root.path().join(input));
            let captured = environment.config_stamps();
            std::fs::remove_file(&bridge).unwrap();
            std::os::unix::fs::symlink(&second, &bridge).unwrap();
            assert!(captured.affected_by(&canonical_root.join("x/b")), "{input}");
            assert!(captured.affected_by(&first.join("pubspec.yaml")));
            assert_ne!(captured, environment.config_stamps());
            std::fs::remove_file(&bridge).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn lib_symlink_retargeting_invalidates_environment_package_roots() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let first = root.path().join("first");
        let second = root.path().join("second");
        for directory in [&package, &first, &second] {
            std::fs::create_dir(directory).unwrap();
        }
        let lib = package.join("lib");
        std::os::unix::fs::symlink(&first, &lib).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: package\n").unwrap();
        let source = package.join("source.dart");
        let mut environment =
            AnalysisEnvironment::discover_with_sdk_locator([source.as_path()], |_| None);
        let stamps = environment.config_stamps();
        assert!(stamps.affected_by(&lib));
        assert!(stamps.affected_by(&first));
        std::fs::remove_file(&lib).unwrap();
        std::os::unix::fs::symlink(&second, &lib).unwrap();
        assert_ne!(stamps, environment.config_stamps());
        assert_eq!(
            environment.package(&source).unwrap().identity.lib_root,
            std::fs::canonicalize(&first).unwrap()
        );
        environment.reset();
        environment.extend([source.as_path()]);
        assert_eq!(
            environment.package(&source).unwrap().identity.lib_root,
            std::fs::canonicalize(&second).unwrap()
        );
    }

    #[test]
    fn attaching_environment_replaces_standalone_package_facts() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.dart");
        let manifest = root.path().join("pubspec.yaml");
        std::fs::write(&manifest, "name: standalone\n").unwrap();
        let parsed = parsed();
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(&source, &config, &parsed);
        assert_eq!(ctx.package().unwrap().name, "standalone");
        std::fs::write(&manifest, "name: environment\n").unwrap();
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([source.as_path()], |_| None);
        let ctx = ctx.with_environment(Some(&environment));
        assert_eq!(ctx.package().unwrap().name, "environment");
    }

    #[cfg(unix)]
    #[test]
    fn canonical_declarations_use_the_environment_package_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let alias = root.path().join("alias");
        std::fs::create_dir_all(package.join("lib")).unwrap();
        std::os::unix::fs::symlink(&package, &alias).unwrap();
        let manifest = package.join("pubspec.yaml");
        std::fs::write(&manifest, "name: package\n").unwrap();
        let source = alias.join("lib/source.dart");
        let declaration = package.join("lib/declaration.dart");
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([source.as_path()], |_| None);
        std::fs::remove_file(&manifest).unwrap();
        let parsed = parsed();
        let config = FalconConfig::default();
        let ctx =
            AnalyzeContext::new(&source, &config, &parsed).with_environment(Some(&environment));
        assert!(ctx.in_same_package(&declaration));
        assert!(ctx.lib_dir_package().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn saved_and_virtual_symlink_aliases_retain_package_snapshots() {
        for saved in [false, true] {
            for workspace_alias in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let package = root.path().join("package");
                let real_sources = package.join("lib/real");
                std::fs::create_dir_all(&real_sources).unwrap();
                let alias = if workspace_alias {
                    let alias = root.path().join("workspace");
                    std::os::unix::fs::symlink(&package, &alias).unwrap();
                    alias.join("lib/real")
                } else {
                    let alias = package.join("lib/alias");
                    std::os::unix::fs::symlink(&real_sources, &alias).unwrap();
                    alias
                };
                let source = alias.join("source.dart");
                if saved {
                    std::fs::write(&source, "").unwrap();
                }
                let manifest = package.join("pubspec.yaml");
                std::fs::write(&manifest, "name: package\n").unwrap();
                let declaration = real_sources.join("declaration.dart");
                let environment =
                    AnalysisEnvironment::discover_with_sdk_locator([source.as_path()], |_| None);
                std::fs::remove_file(&manifest).unwrap();
                let parsed = parsed();
                let config = FalconConfig::default();
                let ctx = AnalyzeContext::new(&source, &config, &parsed)
                    .with_environment(Some(&environment));
                assert!(
                    ctx.in_same_package(&declaration),
                    "saved={saved}, workspace={workspace_alias}"
                );
                assert!(ctx.lib_dir_package().is_some());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_lib_sources_do_not_claim_unowned_canonical_declarations() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let sources = root.path().join("sources");
        std::fs::create_dir(&package).unwrap();
        std::fs::create_dir(&sources).unwrap();
        std::os::unix::fs::symlink(&sources, package.join("lib")).unwrap();
        std::fs::write(package.join("pubspec.yaml"), "name: package\n").unwrap();
        let source = package.join("lib/unsaved.dart");
        let declaration = sources.join("declaration.dart");
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([source.as_path()], |_| None);
        let parsed = parsed();
        let config = FalconConfig::default();
        let ctx =
            AnalyzeContext::new(&source, &config, &parsed).with_environment(Some(&environment));
        assert!(ctx.lib_dir_package().is_some());
        assert!(!ctx.in_same_package(&declaration));
    }

    #[test]
    fn nested_invalid_manifests_keep_their_own_package_boundary() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("nested");
        std::fs::create_dir_all(nested.join("lib")).unwrap();
        std::fs::write(root.path().join("pubspec.yaml"), "name: outer\n").unwrap();
        let manifest = nested.join("pubspec.yaml");
        std::fs::write(&manifest, "name: inner\nbroken: [\n").unwrap();
        let facts = FilePackage::discover(&nested.join("lib/source.dart"));
        assert_eq!(facts.identity().unwrap().name, "inner");
        assert!(facts.metadata().unwrap().parsed_name().is_none());
        assert!(!facts.metadata().unwrap().checks_flutter_web_imports());
        std::fs::write(&manifest, [0xff]).unwrap();
        assert_eq!(
            FilePackage::discover(&nested.join("lib/source.dart"))
                .identity()
                .unwrap()
                .name,
            ""
        );
    }

    #[test]
    fn flutter_web_policy_uses_parsed_package_metadata() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        for (source, expected) in [
            ("name: plain\n", false),
            ("dependencies:\n  flutter:\n    sdk: flutter\n", true),
            (
                "dependencies:\n  flutter:\n    sdk: flutter\nflutter:\n  plugin:\n    platforms:\n      web: {}\n",
                false,
            ),
        ] {
            std::fs::write(&manifest, source).unwrap();
            assert_eq!(
                PackageMetadata::load(&manifest).checks_flutter_web_imports(),
                expected
            );
        }
    }
}
