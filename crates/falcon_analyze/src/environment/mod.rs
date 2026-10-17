//! The analysis environment of a set of project files: their package
//! configurations, language versions, the Dart SDK, and the external (SDK and
//! dependency) libraries they import.

mod external;
mod package_config;
mod sdk;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use external::{
    ExternalFile, ExternalLibraries, ExternalLibrary, ExternalOrigin, ExternalTarget,
};
pub use falcon_syntax::language_version::{Feature, LanguageVersion};
pub use package_config::{ConfiguredPackage, PackageConfig, percent_decode, percent_encode};
pub use sdk::{DartSdk, Embedder};

/// One package configuration and the SDK its packages are analyzed against.
#[derive(Debug, Clone)]
struct Context {
    config: PackageConfig,
    sdk: Option<DartSdk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentCacheKey {
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
    roots: Vec<(PathBuf, PathBuf)>,
    sdk_environment: [Option<std::ffi::OsString>; 3],
}

impl EnvironmentCacheKey {
    /// File events also represent replacement or deletion of an input's parent.
    pub fn affected_by(&self, path: &Path) -> bool {
        self.files.iter().any(|(input, _)| input.starts_with(path))
            || self
                .roots
                .iter()
                .any(|(raw, resolved)| raw.starts_with(path) || resolved.starts_with(path))
    }
}

/// Package configurations, SDKs and language versions for a set of files.
///
/// Built once per run (or per LSP workspace state) from the analyzed paths;
/// the empty environment ([`Self::default`]) knows no packages and no SDK, so
/// every file gets [`LanguageVersion::CURRENT`] (or its `// @dart` override)
/// and no external library is loaded.
#[derive(Debug, Clone)]
pub struct AnalysisEnvironment {
    contexts: Vec<Context>,
    /// The context owning each analyzed directory (`None`: no configuration).
    directories: HashMap<PathBuf, Option<usize>>,
    /// The SDK for files outside any package configuration, once located.
    default_sdk: Option<Option<DartSdk>>,
    sdk_locator: fn(Option<&PackageConfig>) -> Option<DartSdk>,
    config_candidates: std::collections::BTreeSet<PathBuf>,
    /// Uncanonicalized package/SDK roots, so retargeted symlinks rediscover contexts.
    root_candidates: std::collections::BTreeSet<PathBuf>,
    mutable_roots: std::collections::BTreeSet<PathBuf>,
}

impl Default for AnalysisEnvironment {
    fn default() -> Self {
        Self::with_sdk_locator(DartSdk::locate)
    }
}

impl AnalysisEnvironment {
    /// Use an explicit SDK discovery policy, including SDK-free test fixtures.
    pub fn with_sdk_locator(locator: fn(Option<&PackageConfig>) -> Option<DartSdk>) -> Self {
        Self {
            contexts: Vec::new(),
            directories: HashMap::new(),
            default_sdk: None,
            sdk_locator: locator,
            config_candidates: std::collections::BTreeSet::new(),
            root_candidates: std::collections::BTreeSet::new(),
            mutable_roots: std::collections::BTreeSet::new(),
        }
    }

    /// Clear discovered contexts while retaining the SDK discovery policy.
    pub fn reset(&mut self) {
        *self = Self::with_sdk_locator(self.sdk_locator);
    }

    /// Discover the nearest `package_config.json` of every path and the SDK
    /// each configuration is analyzed against.
    pub fn discover<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Self {
        let mut environment = Self::default();
        environment.extend(paths);
        environment
    }

    /// Discover packages with an explicit SDK discovery policy.
    pub fn discover_with_sdk_locator<'a>(
        paths: impl IntoIterator<Item = &'a Path>,
        locator: fn(Option<&PackageConfig>) -> Option<DartSdk>,
    ) -> Self {
        let mut environment = Self::with_sdk_locator(locator);
        environment.extend(paths);
        environment
    }

    /// Discover the configurations of paths in directories not seen before;
    /// known directories and contexts are kept, so context indices stay valid.
    pub fn extend<'a>(&mut self, paths: impl IntoIterator<Item = &'a Path>) {
        for path in paths {
            let directory = package_config::absolute(path.parent().unwrap_or(Path::new("")));
            if self.directories.contains_key(&directory) {
                continue;
            }
            for ancestor in directory.ancestors() {
                self.config_candidates
                    .insert(ancestor.join(".dart_tool/package_config.json"));
                self.config_candidates.insert(ancestor.join("pubspec.lock"));
                self.config_candidates.insert(ancestor.join("pubspec.yaml"));
                self.config_candidates
                    .insert(ancestor.join("pubspec_overrides.yaml"));
            }
            let context = PackageConfig::discover(&directory).and_then(|config_path| {
                if let Some(index) = self
                    .contexts
                    .iter()
                    .position(|context| context.config.path == config_path)
                {
                    return Some(index);
                }
                if let Some(raw) = std::fs::read_to_string(&config_path)
                    .ok()
                    .and_then(|text| PackageConfig::parse(&config_path, &text))
                {
                    let lock = config_path
                        .parent()
                        .and_then(Path::parent)
                        .and_then(|root| std::fs::read_to_string(root.join("pubspec.lock")).ok())
                        .and_then(|source| serde_yaml::from_str::<serde_yaml::Value>(&source).ok());
                    let project_root = config_path.parent().and_then(Path::parent);
                    for package in raw.packages {
                        let hosted = lock
                            .as_ref()
                            .and_then(|lock| lock.get("packages"))
                            .and_then(|packages| packages.get(package.name.as_str()))
                            .and_then(|package| package.get("source"))
                            .and_then(serde_yaml::Value::as_str)
                            == Some("hosted");
                        let resolved = crate::resolve::canonical_or_lexical(&package.root);
                        if !hosted
                            && project_root.is_none_or(|root| {
                                resolved != crate::resolve::canonical_or_lexical(root)
                            })
                        {
                            self.mutable_roots.insert(resolved);
                            self.mutable_roots.insert(package.root);
                        }
                    }
                    if let Some(flutter) = raw.flutter_root {
                        let sdk_root = flutter.join("bin/cache/dart-sdk");
                        self.config_candidates
                            .insert(sdk_root.join("lib/libraries.json"));
                        self.config_candidates.insert(sdk_root.join("version"));
                        self.root_candidates.insert(sdk_root);
                        self.root_candidates.insert(flutter);
                    }
                }
                let config = PackageConfig::load(&config_path)?;
                let mut sdk = (self.sdk_locator)(Some(&config));
                if let (Some(sdk), Some(embedder)) = (&mut sdk, Embedder::from_config(&config)) {
                    sdk.apply_embedder(&embedder);
                }
                if let Some(sdk) = &sdk {
                    self.config_candidates
                        .insert(sdk.root.join("lib/libraries.json"));
                    self.config_candidates.insert(sdk.root.join("version"));
                    self.root_candidates.insert(sdk.root.clone());
                }
                self.contexts.push(Context { config, sdk });
                Some(self.contexts.len() - 1)
            });
            if context.is_none() && self.default_sdk.is_none() {
                self.default_sdk = Some((self.sdk_locator)(None));
                if let Some(Some(sdk)) = &self.default_sdk {
                    self.config_candidates
                        .insert(sdk.root.join("lib/libraries.json"));
                    self.config_candidates.insert(sdk.root.join("version"));
                    self.root_candidates.insert(sdk.root.clone());
                }
            }
            self.directories.insert(directory, context);
        }
    }

    /// Whether every path's directory has been discovered.
    pub fn covers<'a>(&self, mut paths: impl Iterator<Item = &'a Path>) -> bool {
        paths.all(|path| {
            let directory = package_config::absolute(path.parent().unwrap_or(Path::new("")));
            self.directories.contains_key(&directory)
        })
    }

    /// Capture exact resolution inputs on discovery or a configuration event.
    /// This is deliberately not polled on project text edits: versioned hosted
    /// packages and SDK sources are immutable, while mutable roots use LSP events.
    pub fn config_stamps(&self) -> EnvironmentCacheKey {
        let mut roots = self.root_candidates.clone();
        roots.extend(sdk::discovery_inputs());
        roots.extend(self.mutable_roots.iter().cloned());
        let mut files = self.config_candidates.clone();
        for root in &roots {
            files.insert(root.join("version"));
            files.insert(root.join("lib/libraries.json"));
        }
        EnvironmentCacheKey {
            files: files
                .into_iter()
                .map(|path| {
                    let contents = std::fs::read(&path).ok();
                    (path, contents)
                })
                .collect(),
            roots: roots
                .into_iter()
                .map(|path| {
                    let resolved = crate::resolve::canonical_or_lexical(&path);
                    (path, resolved)
                })
                .collect(),
            sdk_environment: ["DART_SDK", "FLUTTER_ROOT", "PATH"].map(std::env::var_os),
        }
    }

    /// Raw roots of path/git/unknown packages, including absent generated files.
    /// A lockfile's hosted source is the only immutable package classification.
    pub fn watched_dependency_roots(&self) -> impl Iterator<Item = &Path> {
        self.mutable_roots.iter().map(PathBuf::as_path)
    }

    /// The package configuration governing `path`, if any.
    pub fn package_config(&self, path: &Path) -> Option<&PackageConfig> {
        self.config_in(self.context_index(path))
    }

    /// The context of an analyzed file's directory; files in directories not
    /// seen by [`Self::discover`] have none.
    fn context_index(&self, path: &Path) -> Option<usize> {
        let directory = package_config::absolute(path.parent().unwrap_or(Path::new("")));
        self.directories.get(&directory).copied().flatten()
    }

    pub(crate) fn config_in(&self, context: Option<usize>) -> Option<&PackageConfig> {
        context.map(|index| &self.contexts[index].config)
    }

    pub(crate) fn sdk_in(&self, context: Option<usize>) -> Option<&DartSdk> {
        match context {
            Some(index) => self.contexts[index].sdk.as_ref(),
            None => self.default_sdk.as_ref()?.as_ref(),
        }
    }

    /// The SDK governing `path`.
    pub fn sdk(&self, path: &Path) -> Option<&DartSdk> {
        self.sdk_in(self.context_index(path))
    }

    /// The language version of the file at `path` with contents `source`: a
    /// leading `// @dart = X.Y` comment, else its package's `languageVersion`,
    /// else [`LanguageVersion::CURRENT`] (the analyzer's default).
    pub fn language_version(&self, path: &Path, source: &str) -> LanguageVersion {
        LanguageVersion::file_override(source)
            .or_else(|| self.package_config(path)?.language_version(path))
            .unwrap_or(LanguageVersion::CURRENT)
    }

    /// Whether any analyzed file has an SDK to load external libraries from.
    pub fn has_sdk(&self) -> bool {
        matches!(self.default_sdk, Some(Some(_)))
            || self.contexts.iter().any(|context| context.sdk.is_some())
    }

    /// The package language version of the file at `path`, or `None` when no
    /// package configuration covers it (see [`crate::AnalyzeContext`]).
    pub fn package_language_version(&self, path: &Path) -> Option<LanguageVersion> {
        self.package_config(path)?.language_version(path)
    }
}
