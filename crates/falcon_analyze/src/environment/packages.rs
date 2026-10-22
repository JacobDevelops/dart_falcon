//! Pubspec discovery and immutable package facts shared by file rules.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{PackageIdentity, canonical_or_lexical};

/// A package manifest as observed when the analysis environment was discovered.
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    pub manifest: PathBuf,
    pub identity: PackageIdentity,
    root: PathBuf,
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
        (canonical != directory && canonical.starts_with(&self.root)).then_some(canonical)
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
        let package =
            enclosing_pubspec(path).map(|manifest| Arc::new(PackageMetadata::load(&manifest)));
        Self::new(path, package)
    }

    pub(super) fn new(path: &Path, package: Option<Arc<PackageMetadata>>) -> Self {
        if package.is_none() {
            return Self {
                package: None,
                in_lib: false,
                directory: None,
            };
        }
        let lexical = canonical_or_lexical(&super::package_config::absolute(path));
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
    if path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml") && path.is_file() {
        return Some(path);
    }
    path.parent()?
        .ancestors()
        .map(|directory| directory.join("pubspec.yaml"))
        .find(|manifest| manifest.is_file())
}

pub fn canonical_package_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        let path = canonical_or_lexical(path);
        let Some(manifest) = enclosing_pubspec(&path) else {
            return path;
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
            first
        );
        environment.reset();
        environment.extend([source.as_path()]);
        assert_eq!(
            environment.package(&source).unwrap().identity.lib_root,
            second
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
