//! falcon.json configuration schema and loader (biome 2.x-shaped).
//!
//! `FalconConfig` is the contract; every field and its default is documented
//! here. File rules are grouped by category under `linter.rules`; enablement is
//! resolved from an explicit per-rule level, the `recommended` preset, and
//! per-domain gating (see [`LinterConfig::resolve_rule`]). Cross-file rules are a
//! separate feature under `cross_file.rules`, resolved the same way minus domains
//! (see [`CrossFileConfig::resolve_rule`]).

mod metadata;
pub use metadata::{MAX_METADATA_BYTES, metadata_content, read_metadata_bytes, read_metadata_text};

use glob::Pattern;
use serde::de::{self, DeserializeOwned, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// Top-level falcon.json configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FalconConfig {
    /// Optional JSON-schema URL; ignored by the linter.
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// File inclusion/exclusion globs.
    pub files: FilesConfig,
    /// Linter enablement, rule levels, and domain gating.
    pub linter: LinterConfig,
    /// Cross-file rule enablement and levels. A separate feature from `linter`:
    /// these rules reason across the whole file set (unused files, unused code,
    /// call-site nullability) and run in the CLI and LSP cross-file passes.
    /// Accepts the legacy keys `project` and `cross_file` as deserialization aliases.
    #[serde(rename = "cross-file", alias = "project", alias = "cross_file")]
    pub cross_file: CrossFileConfig,
    /// Per-path rule re-configuration (biome `overrides`). Each entry re-patches
    /// the base linter and/or cross-file resolution for files its `includes` match;
    /// later entries win over earlier ones, and all win over the base config.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    /// Maximum number of printed diagnostics. Defaults to None (unlimited).
    /// Accepts the legacy key `max_errors` as a deserialization alias.
    #[serde(
        rename = "max-errors",
        alias = "max_errors",
        deserialize_with = "optional_integer::<_, 1>"
    )]
    pub max_errors: Option<usize>,
    /// Optional diagnostic baseline path. Relative paths use the configuration
    /// files root; the CLI's explicit baseline path takes precedence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<PathBuf>,
    /// Fail checks with warning diagnostics as well as errors. Defaults to false.
    #[serde(rename = "error-on-warnings")]
    pub error_on_warnings: bool,
    /// Runtime-only replacement configurations for workspace package directories.
    #[serde(skip)]
    pub package_configs: Vec<FalconConfig>,
}

/// Accept JSON Schema integers, including whole-number float representations.
///
/// # Errors
///
/// Rejects non-numbers, fractional values, and values outside the `usize` range.
pub fn integer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
    integer_with_range_minimum::<D, 0>(deserializer)
}

fn integer_with_range_minimum<'de, D: Deserializer<'de>, const RANGE_MIN: usize>(
    deserializer: D,
) -> Result<usize, D::Error> {
    use serde::de::Error;

    let number = serde_json::Number::deserialize(deserializer)?;
    let range_error = || {
        D::Error::custom(format_args!(
            "expected an integer in the allowed range {RANGE_MIN}..={}",
            usize::MAX
        ))
    };
    if let Some(value) = number.as_u64() {
        return usize::try_from(value).map_err(|_| range_error());
    }
    let value = number.as_f64().ok_or_else(range_error)?;
    if !(0.0..18_446_744_073_709_551_616.0).contains(&value) || value.fract() != 0.0 {
        return Err(range_error());
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the preceding range and integral-value checks make this conversion exact"
    )]
    let value = value as u64;
    usize::try_from(value).map_err(|_| range_error())
}

/// Deserialize an optional JSON Schema integer, preserving null as `None`.
///
/// `RANGE_MIN` controls the diagnostic's lower bound; callers validate their
/// minimum separately so they can provide context-specific errors for zero.
///
/// # Errors
///
/// Rejects non-null values that [`integer`] rejects.
pub fn optional_integer<'de, D: Deserializer<'de>, const RANGE_MIN: usize>(
    deserializer: D,
) -> Result<Option<usize>, D::Error> {
    #[derive(Deserialize)]
    struct Integer<const RANGE_MIN: usize>(
        #[serde(deserialize_with = "integer_with_range_minimum::<_, RANGE_MIN>")] usize,
    );

    Option::<Integer<RANGE_MIN>>::deserialize(deserializer)
        .map(|value| value.map(|integer| integer.0))
}

/// One `overrides` entry: a path filter plus a partial linter that re-patches
/// the base resolution for matching files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Override {
    /// Glob patterns selecting the files this override applies to. Same syntax
    /// as `files.includes`: plain entries are positive includes, `!`-prefixed
    /// entries are exclusions. Relative globs resolve from the config directory.
    pub includes: PathFilter,
    /// Partial linter (file-rule) configuration applied to matching files. Rule
    /// levels, per-rule `options`, and an optional `enabled` master switch are
    /// honored. Options replace the base rule's options for matching files (see
    /// [`FalconConfig::rule_options_data_for`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linter: Option<OverrideRules>,
    /// Partial cross-file rule configuration applied to matching files. Same
    /// shape and semantics as `linter`, resolved against `cross_file.rules`.
    /// Accepts the legacy keys `project` and `cross_file` as deserialization aliases.
    #[serde(
        rename = "cross-file",
        alias = "project",
        alias = "cross_file",
        skip_serializing_if = "Option::is_none"
    )]
    pub cross_file: Option<OverrideRules>,
}

/// The partial rule block permitted inside an override: a master switch and
/// per-group rule levels. Shared by the override's `linter` and `cross_file`
/// sections. Domains are intentionally omitted — overrides are rule-level only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OverrideRules {
    /// When `Some(false)`, every rule is disabled for matching files (unless a
    /// later override re-enables one). `None` leaves the base enablement intact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Rule levels grouped by category (same shape as `linter.rules`).
    pub rules: Rules,
}

impl Override {
    /// Match walked paths; invalid globs are ignored.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        self.includes.admits(path)
    }
}

/// Compiled globs. Invalid patterns are warned about and skipped; exclusions
/// win, and no valid positive globs means every path is included.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(from = "Vec<String>")]
pub struct PathFilter {
    patterns: Vec<String>,
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
    root: Option<Arc<PathRoot>>,
    warnings: Vec<String>,
    package_filters: Vec<PathFilter>,
    declared_member_patterns: Vec<Pattern>,
}

#[derive(Debug)]
struct PathRoot {
    canonical: PathBuf,
    lexical: PathBuf,
    working_dir: PathBuf,
    #[cfg(test)]
    canonicalizations: std::sync::atomic::AtomicUsize,
}

/// A single fresh filesystem identity shared by every filter in one lookup.
struct MatchPath<'a> {
    original: &'a str,
    normalized: std::borrow::Cow<'a, Path>,
    canonical: Option<PathBuf>,
}

impl PathRoot {
    fn canonical_parent(
        parent: &Path,
        #[cfg(test)] canonicalizations: &std::sync::atomic::AtomicUsize,
    ) -> Option<PathBuf> {
        // Aliases can change while an editor configuration stays loaded.
        // Resolve them afresh, including missing descendants of a link.
        for ancestor in parent.ancestors() {
            #[cfg(test)]
            canonicalizations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(canonical) = std::fs::canonicalize(ancestor) {
                return Some(canonical.join(parent.strip_prefix(ancestor).unwrap()));
            }
        }
        None
    }
}

fn strip_root<'a>(path: &'a Path, base: &Path) -> Option<&'a Path> {
    if let Ok(suffix) = path.strip_prefix(base) {
        return Some(suffix);
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut path_components = path.components();
        let mut base_components = base.components();
        if let (Some(Component::Prefix(path_prefix)), Some(Component::Prefix(base_prefix))) =
            (path_components.next(), base_components.next())
        {
            let equivalent = match (path_prefix.kind(), base_prefix.kind()) {
                (Prefix::Disk(a), Prefix::VerbatimDisk(b))
                | (Prefix::VerbatimDisk(a), Prefix::Disk(b)) => a.eq_ignore_ascii_case(&b),
                (Prefix::UNC(a, b), Prefix::VerbatimUNC(c, d))
                | (Prefix::VerbatimUNC(a, b), Prefix::UNC(c, d)) => a == c && b == d,
                _ => false,
            };
            if equivalent {
                return path_components
                    .as_path()
                    .strip_prefix(base_components.as_path())
                    .ok();
            }
        }
    }
    None
}

impl From<Vec<String>> for PathFilter {
    fn from(patterns: Vec<String>) -> Self {
        let mut filter = Self {
            patterns,
            ..Self::default()
        };
        filter.compile();
        filter
    }
}

impl Serialize for PathFilter {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.patterns.serialize(serializer)
    }
}

impl PathFilter {
    #[must_use]
    pub fn from_includes(patterns: &[String]) -> Self {
        patterns.to_vec().into()
    }

    /// Preserve potential member sources before inaccessible member configs can be loaded.
    pub fn set_declared_member_patterns(&mut self, patterns: &[String]) {
        let directory = self
            .root
            .as_ref()
            .map(|root| format!("{}/", root.lexical.display()));
        self.declared_member_patterns = patterns
            .iter()
            .filter_map(|pattern| Pattern::new(pattern).ok())
            .filter(|pattern| {
                directory
                    .as_ref()
                    .is_none_or(|directory| !matches_path(pattern, directory))
            })
            .collect();
        for filter in &mut self.package_filters {
            filter.set_declared_member_patterns(patterns);
        }
    }

    pub fn extend_excludes(&mut self, patterns: &[String]) {
        if !self.package_filters.is_empty() {
            let rooted: Vec<_> = patterns
                .iter()
                .map(|pattern| {
                    self.root.as_ref().map_or_else(
                        || pattern.clone(),
                        |root| {
                            if Path::new(pattern).is_absolute() {
                                pattern.clone()
                            } else {
                                Path::new(&Pattern::escape(&root.lexical.to_string_lossy()))
                                    .join(pattern)
                                    .to_string_lossy()
                                    .into_owned()
                            }
                        },
                    )
                })
                .collect();
            for filter in &mut self.package_filters {
                filter.extend_excludes(&rooted);
            }
        }
        self.patterns
            .extend(patterns.iter().map(|raw| format!("!{raw}")));
        self.compile();
    }

    /// Exclude metadata-discovery failures independently of package replacements.
    pub fn exclude_paths(&mut self, paths: &[PathBuf]) {
        let patterns: Vec<_> = paths
            .iter()
            .map(|path| Pattern::escape(&path.to_string_lossy()))
            .collect();
        self.extend_excludes(&patterns);
    }

    pub fn set_base_dir(&mut self, base: &Path) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let lexical = normalize_path(&cwd.join(base));
        let canonical = std::fs::canonicalize(&lexical).unwrap_or_else(|_| lexical.clone());
        self.set_root(Arc::new(PathRoot {
            canonical,
            lexical,
            working_dir: cwd,
            #[cfg(test)]
            canonicalizations: std::sync::atomic::AtomicUsize::new(0),
        }));
    }

    fn set_root(&mut self, root: Arc<PathRoot>) {
        self.root = Some(root);
        self.compile();
    }

    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    fn compile(&mut self) {
        self.include.clear();
        self.exclude.clear();
        self.warnings.clear();
        for pattern in &self.patterns {
            let (is_exclusion, raw) = match pattern.strip_prefix('!') {
                Some(raw) => (true, raw),
                None => (false, pattern.as_str()),
            };
            let resolved = if Path::new(raw).is_absolute() {
                let path = normalize_path(Path::new(raw));
                let path = if let Some(root) = &self.root
                    && let Some(suffix) = self.relative_to_base(&path)
                {
                    Path::new(&Pattern::escape(&root.lexical.to_string_lossy())).join(suffix)
                } else {
                    path
                };
                Some(path.to_string_lossy().into_owned())
            } else {
                self.root.as_ref().map(|root| {
                    let base = &root.lexical;
                    let escaped = Pattern::escape(&base.to_string_lossy());
                    let path = normalize_path(&Path::new(&escaped).join(raw));
                    path.to_string_lossy().into_owned()
                })
            };
            match Pattern::new(resolved.as_deref().unwrap_or(raw)) {
                Ok(pattern) => {
                    if is_exclusion {
                        self.exclude.push(pattern);
                    } else {
                        self.include.push(pattern);
                    }
                }
                Err(error) => self
                    .warnings
                    .push(format!("invalid path pattern `{raw}`: {error}")),
            }
        }
        if self
            .patterns
            .iter()
            .any(|pattern| pattern == "**" || pattern == "**/*")
        {
            self.include.clear();
        }
    }

    fn relative_to_base(&self, path: &Path) -> Option<PathBuf> {
        let root = self.root.as_ref()?;
        // Preserve internal aliases' physical root-relative spelling and
        // external links' lexical spelling.
        if let Some(parent) = path.parent()
            && let Some(mut canonical) = PathRoot::canonical_parent(
                parent,
                #[cfg(test)]
                &root.canonicalizations,
            )
        {
            canonical.push(path.file_name()?);
            if let Some(suffix) = strip_root(&canonical, &root.canonical) {
                return Some(suffix.to_path_buf());
            }
        }
        strip_root(path, &root.lexical)
            .or_else(|| strip_root(path, &root.canonical))
            .map(Path::to_path_buf)
    }

    fn match_path<'a>(&self, path: &'a str) -> MatchPath<'a> {
        let input = Path::new(path);
        let normalized = match &self.root {
            Some(root) if !input.is_absolute() => {
                std::borrow::Cow::Owned(normalize_path(&root.working_dir.join(input)))
            }
            Some(_)
                if path.contains("//")
                    || path.contains("/./")
                    || path.contains("/../")
                    || path.ends_with("/.")
                    || path.ends_with("/..")
                    || path.ends_with('/') =>
            {
                std::borrow::Cow::Owned(normalize_path(input))
            }
            _ => std::borrow::Cow::Borrowed(input),
        };
        let canonical = self.root.as_deref().and_then(
            |PathRoot {
                 #[cfg(test)]
                 canonicalizations,
                 ..
             }| {
                let mut parent = PathRoot::canonical_parent(
                    normalized.parent()?,
                    #[cfg(test)]
                    canonicalizations,
                )?;
                parent.push(normalized.file_name()?);
                Some(parent)
            },
        );
        MatchPath {
            original: path,
            normalized,
            canonical,
        }
    }

    fn matching_path<'a>(&self, path: &'a MatchPath<'_>) -> std::borrow::Cow<'a, str> {
        let Some(root) = &self.root else {
            return path.original.into();
        };
        let suffix = path
            .canonical
            .as_ref()
            .and_then(|canonical| strip_root(canonical, &root.canonical))
            .or_else(|| strip_root(&path.normalized, &root.lexical))
            .or_else(|| strip_root(&path.normalized, &root.canonical));
        if let Some(suffix) = suffix
            && strip_root(&path.normalized, &root.lexical) != Some(suffix)
        {
            return if suffix.as_os_str().is_empty() {
                root.lexical.to_string_lossy().into_owned()
            } else {
                root.lexical.join(suffix).to_string_lossy().into_owned()
            }
            .into();
        }
        path.normalized.to_string_lossy()
    }

    fn contains_path(&self, path: &MatchPath<'_>) -> bool {
        self.root.as_ref().is_some_and(|root| {
            path.canonical
                .as_ref()
                .is_some_and(|canonical| strip_root(canonical, &root.canonical).is_some())
                || strip_root(&path.normalized, &root.lexical).is_some()
                || strip_root(&path.normalized, &root.canonical).is_some()
        })
    }

    fn package_filter(&self, path: &MatchPath<'_>) -> Option<&Self> {
        self.package_filters
            .iter()
            .filter(|filter| filter.contains_path(path))
            .max_by_key(|filter| {
                filter
                    .root
                    .as_ref()
                    .map(|root| root.lexical.components().count())
            })
    }

    #[must_use]
    pub fn excludes(&self, path: &str) -> bool {
        self.excludes_identity(&self.match_path(path))
    }

    fn excludes_identity(&self, path: &MatchPath<'_>) -> bool {
        if let Some(filter) = self.package_filter(path) {
            return filter.excludes_identity(path);
        }
        let path = self.matching_path(path);
        self.excludes_resolved(&path)
    }

    fn excludes_resolved(&self, path: &str) -> bool {
        Path::new(path).ancestors().any(|ancestor| {
            let ancestor = ancestor.to_string_lossy();
            self.exclude
                .iter()
                .any(|pattern| matches_path(pattern, &ancestor))
        })
    }

    /// Resolve a traversal root once while retaining lexical external links.
    ///
    /// Descendants reached without following links can then be matched with
    /// [`Self::excludes_walked_directory`] using this spelling.
    #[must_use]
    pub fn resolve_walk_root(&self, path: &Path) -> PathBuf {
        let Some(root) = &self.root else {
            return path.to_path_buf();
        };
        let normalized = normalize_path(&root.working_dir.join(path));
        for ancestor in normalized.ancestors() {
            if let Some(canonical) = PathRoot::canonical_parent(
                ancestor,
                #[cfg(test)]
                &root.canonicalizations,
            ) && let Some(suffix) = strip_root(&canonical, &root.canonical)
                && let Ok(remainder) = normalized.strip_prefix(ancestor)
            {
                return root.lexical.join(suffix).join(remainder);
            }
        }
        normalized
    }

    /// Match a directory reached without following links below the filter root.
    ///
    /// Project walkers can reuse the root identity instead of resolving every
    /// directory again. Paths outside either root spelling retain alias handling.
    #[must_use]
    pub fn excludes_walked_directory(&self, path: &Path) -> bool {
        let Some(root) = &self.root else {
            return self.excludes_directory_path(path.to_string_lossy().into_owned());
        };
        let normalized = normalize_path(&root.working_dir.join(path));
        let Some(suffix) = strip_root(&normalized, &root.lexical)
            .or_else(|| strip_root(&normalized, &root.canonical))
        else {
            return self.excludes_directory(path);
        };
        let canonical = root.canonical.join(suffix);
        if self.package_filters.iter().any(|filter| {
            filter.root.as_ref().is_some_and(|root| {
                (root.canonical != canonical && root.canonical.starts_with(&canonical))
                    || (root.lexical != normalized && root.lexical.starts_with(&normalized))
            })
        }) {
            return false;
        }
        if let Some((filter, path)) = self.walked_package_filter(&normalized, &canonical) {
            return filter.excludes_walked_directory(path);
        }
        let path = if suffix.as_os_str().is_empty() {
            root.lexical.to_string_lossy().into_owned()
        } else {
            root.lexical.join(suffix).to_string_lossy().into_owned()
        };
        self.excludes_directory_path(path)
    }

    fn walked_package_filter<'a, 'p>(
        &'a self,
        normalized: &'p Path,
        canonical: &'p Path,
    ) -> Option<(&'a Self, &'p Path)> {
        self.package_filters
            .iter()
            .filter_map(|filter| {
                let root = filter.root.as_ref()?;
                let path = if canonical.starts_with(&root.canonical)
                    || canonical.starts_with(&root.lexical)
                {
                    canonical
                } else if normalized.starts_with(&root.lexical) {
                    normalized
                } else {
                    return None;
                };
                Some((filter, path))
            })
            .max_by_key(|(filter, _)| filter.root.as_ref().unwrap().lexical.components().count())
    }

    fn walked_file_filter_path(&self, path: &Path) -> (&Self, PathBuf) {
        let Some(root) = &self.root else {
            return (self, path.to_path_buf());
        };
        let normalized = normalize_path(&root.working_dir.join(path));
        let suffix = strip_root(&normalized, &root.lexical)
            .or_else(|| strip_root(&normalized, &root.canonical));
        let canonical = suffix.map_or_else(|| normalized.clone(), |path| root.canonical.join(path));
        if let Some((filter, path)) = self.walked_package_filter(&normalized, &canonical) {
            return filter.walked_file_filter_path(path);
        }
        let path = suffix.map_or_else(|| normalized.clone(), |path| root.lexical.join(path));
        (self, path)
    }

    /// Admit a file whose parent was reached without following directory links.
    ///
    /// The path must use the spelling returned by [`Self::resolve_walk_root`].
    #[must_use]
    pub fn admits_walked_file(&self, path: &Path) -> bool {
        let (filter, path) = self.walked_file_filter_path(path);
        filter.admits_resolved(&path.to_string_lossy())
    }

    /// Exclude a file using an already resolved traversal root.
    #[must_use]
    pub fn excludes_walked_file(&self, path: &Path) -> bool {
        let (filter, path) = self.walked_file_filter_path(path);
        filter.excludes_resolved(&path.to_string_lossy())
    }

    #[must_use]
    pub fn excludes_directory(&self, path: &Path) -> bool {
        let text = path.to_string_lossy();
        let normalized = self
            .root
            .as_ref()
            .map_or(std::borrow::Cow::Borrowed(path), |root| {
                std::borrow::Cow::Owned(normalize_path(&root.working_dir.join(path)))
            });
        let canonical = self.root.as_deref().and_then(
            |PathRoot {
                 #[cfg(test)]
                 canonicalizations,
                 ..
             }| {
                PathRoot::canonical_parent(
                    &normalized,
                    #[cfg(test)]
                    canonicalizations,
                )
            },
        );
        self.excludes_directory_identity(&MatchPath {
            original: &text,
            normalized,
            canonical,
        })
    }

    fn excludes_directory_identity(&self, path: &MatchPath<'_>) -> bool {
        // Replacement configs can admit descendants excluded by the parent.
        if self.package_filters.iter().any(|filter| {
            filter.root.as_ref().is_some_and(|root| {
                path.canonical
                    .as_ref()
                    .is_some_and(|path| root.canonical != *path && root.canonical.starts_with(path))
            })
        }) {
            return false;
        }
        if let Some(filter) = self.package_filter(path) {
            return filter.excludes_directory_identity(path);
        }
        self.excludes_directory_path(self.matching_path(path).into_owned())
    }

    fn excludes_directory_path(&self, mut path: String) -> bool {
        if self.excludes_resolved(&path) {
            return true;
        }
        if !path.ends_with('/') {
            path.push('/');
        }
        self.exclude
            .iter()
            .any(|pattern| matches_path(pattern, &path))
    }

    /// Whether any included file could be a descendant of this directory.
    #[must_use]
    pub fn may_include_descendants(&self, path: &Path) -> bool {
        let text = path.to_string_lossy();
        let identity = self.match_path(&text);
        let resolved = self.matching_path(&identity);
        if self.package_filters.iter().any(|filter| {
            filter.root.as_ref().is_some_and(|root| {
                root.lexical != path && root.lexical.starts_with(resolved.as_ref())
            })
        }) {
            return true;
        }
        if let Some(filter) = self.package_filter(&identity) {
            return filter.may_include_descendants(path);
        }
        if self.excludes_directory_identity(&identity) {
            return false;
        }
        if self.include.is_empty() {
            return true;
        }
        let directory = format!("{}/", resolved.trim_end_matches(std::path::is_separator));
        if self
            .declared_member_patterns
            .iter()
            .any(|pattern| pattern_may_include_descendants(pattern, &directory))
        {
            return true;
        }
        self.include
            .iter()
            .any(|pattern| pattern_may_include_descendants(pattern, &directory))
    }

    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    #[must_use]
    pub fn admits(&self, path: &str) -> bool {
        self.admits_identity(&self.match_path(path))
    }

    fn admits_identity(&self, path: &MatchPath<'_>) -> bool {
        if let Some(filter) = self.package_filter(path) {
            return filter.admits_identity(path);
        }
        let path = self.matching_path(path);
        self.admits_resolved(&path)
    }

    fn admits_resolved(&self, path: &str) -> bool {
        !self.excludes_resolved(path)
            && (self.include.is_empty()
                || self
                    .include
                    .iter()
                    .any(|pattern| matches_path(pattern, path)))
    }
}

fn pattern_may_include_descendants(pattern: &Pattern, directory: &str) -> bool {
    (!pattern.as_str().ends_with(std::path::is_separator) && matches_path(pattern, directory))
        || pattern
            .as_str()
            .match_indices(std::path::is_separator)
            .any(|(end, _)| {
                end + 1 < pattern.as_str().len()
                    && Pattern::new(&pattern.as_str()[..=end])
                        .is_ok_and(|prefix| matches_path(&prefix, directory))
            })
}

fn matches_path(pattern: &Pattern, path: &str) -> bool {
    pattern.matches_with(
        path,
        glob::MatchOptions {
            require_literal_separator: true,
            ..glob::MatchOptions::new()
        },
    )
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            component => result.push(component.as_os_str()),
        }
    }
    result
}

impl FalconConfig {
    /// Attach package replacements while preserving a single shared path filter.
    pub fn set_package_configs(&mut self, configs: Vec<FalconConfig>) {
        self.files.includes.package_filters = configs
            .iter()
            .map(|config| config.files.includes.clone())
            .collect();
        self.package_configs = configs;
    }

    /// Select the nearest package configuration, falling back to this config.
    #[must_use]
    pub fn config_for_path(&self, path: &str) -> &Self {
        self.config_for_identity(&self.files.includes.match_path(path))
    }

    fn config_for_identity(&self, path: &MatchPath<'_>) -> &Self {
        self.package_configs
            .iter()
            .filter(|config| config.files.includes.contains_path(path))
            .max_by_key(|config| config.root_dir().map(|root| root.components().count()))
            .map_or(self, |config| config.config_for_identity(path))
    }

    #[must_use]
    pub fn root_dir(&self) -> Option<&Path> {
        self.files
            .includes
            .root
            .as_ref()
            .map(|root| root.canonical.as_path())
    }

    #[must_use]
    pub fn path_warnings(&self) -> Vec<String> {
        let mut warnings = self.files.includes.warnings.clone();
        for entry in &self.overrides {
            warnings.extend_from_slice(entry.includes.warnings());
        }
        warnings
    }

    /// Resolve paths relative to `base`, sharing the canonical root with overrides.
    ///
    /// # Panics
    ///
    /// Panics if the root include filter has not been initialized.
    pub fn resolve_paths(&mut self, base: &Path) {
        self.files.includes.set_base_dir(base);
        let root = self.files.includes.root.as_ref().unwrap();
        for entry in &mut self.overrides {
            entry.includes.set_root(Arc::clone(root));
        }
    }
}

/// `files.includes`: a mixed list of positive include globs and `!`-prefixed
/// exclusions (biome semantics).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    /// Glob patterns. Entries starting with `!` are exclusions; the rest are
    /// positive includes. Empty means "include everything".
    pub includes: PathFilter,
}

/// `linter`: master switch, rule levels, and domain gating.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LinterConfig {
    /// Master switch. When false, every rule resolves to disabled.
    pub enabled: bool,
    /// Rule levels grouped by category, plus the `recommended` preset.
    pub rules: Rules,
    /// Per-domain gating (e.g. `{"flutter": "recommended"}`). Null means absent.
    #[serde(deserialize_with = "deserialize_domains")]
    pub domains: BTreeMap<String, DomainValue>,
}

fn deserialize_domains<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, DomainValue>, D::Error> {
    Ok(Option::<BTreeMap<String, DomainValue>>::deserialize(deserializer)?.unwrap_or_default())
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Rules::default(),
            domains: BTreeMap::new(),
        }
    }
}

/// `cross_file`: master switch and rule levels for cross-file rules.
///
/// A separate top-level feature from `linter`. Resolution mirrors the linter's
/// per-rule/recommended logic but has **no domain gating** — cross-file rules are
/// not domain-scoped. When `enabled` is false, every cross-file rule resolves off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossFileConfig {
    /// Master switch. When false, every cross-file rule resolves to disabled.
    pub enabled: bool,
    /// Rule levels grouped by category, plus the `recommended` preset.
    pub rules: Rules,
}

impl Default for CrossFileConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Rules::default(),
        }
    }
}

impl CrossFileConfig {
    /// Resolve a cross-file rule's effective severity, or `None` if disabled.
    #[must_use]
    pub fn resolve_rule(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> Option<ResolvedSeverity> {
        self.rules.resolve_rule(self.enabled, group, name, || {
            recommended && self.rules.recommended != Some(false)
        })
    }
}

/// Map an explicit rule level to a resolved severity (`off` → disabled).
#[must_use]
pub fn level_to_severity(level: RulePlainConfiguration) -> Option<ResolvedSeverity> {
    match level {
        RulePlainConfiguration::Off => None,
        RulePlainConfiguration::On | RulePlainConfiguration::Warn => Some(ResolvedSeverity::Warn),
        RulePlainConfiguration::Info => Some(ResolvedSeverity::Info),
        RulePlainConfiguration::Error => Some(ResolvedSeverity::Error),
    }
}

/// `linter.rules`: the `recommended` preset flag plus per-group rule maps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    /// Whether the recommended preset is active. `None` is treated as true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended: Option<bool>,
    /// Group name → (rule name → configuration).
    #[serde(flatten)]
    pub groups: BTreeMap<String, BTreeMap<String, RuleConfiguration>>,
}

impl Rules {
    fn resolve_rule(
        &self,
        enabled: bool,
        group: &str,
        name: &str,
        preset_enabled: impl FnOnce() -> bool,
    ) -> Option<ResolvedSeverity> {
        if !enabled {
            return None;
        }
        if let Some(config) = self.groups.get(group).and_then(|rules| rules.get(name)) {
            return level_to_severity(config.level());
        }
        preset_enabled().then_some(ResolvedSeverity::Warn)
    }
}

/// Raw rule options and their lazily initialized, rule-specific parsed data.
///
/// Serialization includes only the raw JSON value. Cloning an initialized
/// wrapper shares its parsed data; mutating a clone's value clears only its cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleOptions {
    value: serde_json::Value,
    #[serde(skip)]
    parsed: OnceLock<Arc<dyn Any + Send + Sync>>,
}

impl RuleOptions {
    /// The original JSON options.
    pub fn value(&self) -> &serde_json::Value {
        &self.value
    }

    /// Mutate the JSON options, invalidating any previously parsed data.
    pub fn value_mut(&mut self) -> &mut serde_json::Value {
        let _ = self.parsed.take();
        &mut self.value
    }

    /// Initialize the parsed data once and return it on subsequent accesses.
    ///
    /// # Panics
    ///
    /// Panics if this wrapper was already initialized with a different type.
    fn get_or_init<T: Any + Send + Sync>(
        &self,
        initializer: impl FnOnce(&serde_json::Value) -> T,
    ) -> &T {
        self.parsed
            .get_or_init(|| Arc::new(initializer(&self.value)))
            .downcast_ref()
            .expect("rule options initialized with a different parsed type")
    }

    /// Initialize parsed data or a validation error once, sharing either result.
    ///
    /// # Panics
    ///
    /// Panics if this wrapper was already initialized with a different type.
    ///
    /// # Errors
    ///
    /// Returns the initializer's error if parsing fails.
    pub fn get_or_try_init<T: Any + Send + Sync, E: Any + Send + Sync>(
        &self,
        initializer: impl FnOnce(&serde_json::Value) -> Result<T, E>,
    ) -> Result<&T, &E> {
        self.get_or_init(initializer).as_ref()
    }
}

/// Per-rule autofix policy, independent of the diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleFix {
    /// Do not offer a fix for this rule.
    None,
    /// Treat this rule's fixes as safe.
    Safe,
    /// Treat this rule's fixes as unsafe.
    Unsafe,
}

/// A single rule's configuration: either a bare level or a level with options and fix policy.
///
/// Deserialization is hand-written (rather than `#[serde(untagged)]`) so that an
/// invalid level names the offending value and the valid set, instead of the
/// opaque "data did not match any variant of untagged enum".
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum RuleConfiguration {
    /// `"off"`, `"warn"`, etc.
    Plain(RulePlainConfiguration),
    /// `{ "level": "error", "options": { ... } }`.
    WithOptions {
        level: RulePlainConfiguration,
        options: RuleOptions,
        #[serde(skip_serializing_if = "Option::is_none")]
        fix: Option<RuleFix>,
    },
}

impl RuleConfiguration {
    /// The configured level, regardless of form.
    pub fn level(&self) -> RulePlainConfiguration {
        match self {
            RuleConfiguration::Plain(level) | RuleConfiguration::WithOptions { level, .. } => {
                *level
            }
        }
    }

    /// The explicit autofix policy, or `None` to inherit the previous policy.
    pub fn fix(&self) -> Option<RuleFix> {
        match self {
            Self::Plain(_) => None,
            Self::WithOptions { fix, .. } => *fix,
        }
    }

    /// The configured options and parsed cache, or `None` if absent or null.
    pub fn options_data(&self) -> Option<&RuleOptions> {
        match self {
            RuleConfiguration::WithOptions { options, .. } if !options.value().is_null() => {
                Some(options)
            }
            _ => None,
        }
    }
}

/// Parse a rule level string, naming the offending value on failure.
///
/// # Errors
///
/// Returns an error containing `value` when it is not a recognized level.
pub fn level_from_str<E: de::Error>(value: &str) -> Result<RulePlainConfiguration, E> {
    match value {
        "off" => Ok(RulePlainConfiguration::Off),
        "on" => Ok(RulePlainConfiguration::On),
        "info" => Ok(RulePlainConfiguration::Info),
        "warn" => Ok(RulePlainConfiguration::Warn),
        "error" => Ok(RulePlainConfiguration::Error),
        other => Err(E::custom(format!(
            "unknown rule level \"{other}\"; expected off, on, info, warn, error"
        ))),
    }
}

impl<'de> Deserialize<'de> for RuleConfiguration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RuleConfigVisitor;

        impl<'de> Visitor<'de> for RuleConfigVisitor {
            type Value = RuleConfiguration;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a level string or a { level, options, fix } object")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(RuleConfiguration::Plain(level_from_str(value)?))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut level: Option<RulePlainConfiguration> = None;
                let mut options: Option<RuleOptions> = None;
                let mut fix: Option<RuleFix> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "level" => {
                            let raw = map.next_value::<String>()?;
                            level = Some(level_from_str(&raw)?);
                        }
                        "options" => options = Some(map.next_value()?),
                        "fix" => fix = Some(map.next_value()?),
                        _ => {
                            map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }
                let level = level.ok_or_else(|| de::Error::missing_field("level"))?;
                Ok(RuleConfiguration::WithOptions {
                    level,
                    options: options.unwrap_or_default(),
                    fix,
                })
            }
        }

        deserializer.deserialize_any(RuleConfigVisitor)
    }
}

/// A rule level as written in falcon.json.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RulePlainConfiguration {
    /// Disabled.
    Off,
    /// Enabled at the default severity (Warning).
    On,
    Info,
    Warn,
    Error,
}

/// A domain gate value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DomainValue {
    /// Enable every rule in the domain.
    All,
    /// Enable the domain's recommended rules.
    Recommended,
    /// Disable the domain.
    None,
}

/// Resolved severity for an enabled rule. `On` maps to `Warn`. Kept
/// independent of `falcon_diagnostics` so this crate has no dependency on it;
/// `falcon_rules` converts to `falcon_diagnostics::Severity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResolvedSeverity {
    Info,
    Warn,
    Error,
}

/// Borrowed file-rule settings with path overrides matched once for one analysis.
///
/// The configuration cannot change while this view is borrowed. Construct a new
/// view when analyzing another file or after reloading configuration.
pub struct ResolvedFileConfig<'a> {
    config: &'a FalconConfig,
    overrides: Vec<(usize, &'a Override)>,
}

impl ResolvedFileConfig<'_> {
    /// Return effective options without repeating filesystem or glob matching.
    #[must_use]
    pub fn rule_options_data(&self, group: &str, name: &str) -> Option<&RuleOptions> {
        self.options_data(group, name).0
    }

    fn options_data(&self, group: &str, name: &str) -> (Option<&RuleOptions>, Option<usize>) {
        // Last non-null options replace the whole value; level-only overrides
        // and disabled rules still inherit earlier options.
        for &(index, ov) in self.overrides.iter().rev() {
            if let Some(options) = ov
                .linter
                .as_ref()
                .and_then(|block| block.rules.groups.get(group))
                .and_then(|rules| rules.get(name))
                .and_then(RuleConfiguration::options_data)
            {
                return (Some(options), Some(index));
            }
        }
        (
            self.config
                .linter
                .rules
                .groups
                .get(group)
                .and_then(|rules| rules.get(name))
                .and_then(RuleConfiguration::options_data),
            None,
        )
    }

    /// Locate the same effective options used by [`Self::rule_options_data`].
    #[must_use]
    pub fn rule_options_location(&self, group: &str, name: &str) -> Vec<ConfigPathSegment> {
        options_location(self.options_data(group, name).1, "linter", group, name)
    }

    /// Resolve severity using the same ordered override snapshot as options.
    #[must_use]
    pub fn resolve_rule(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> Option<ResolvedSeverity> {
        self.config.resolve_overrides(
            self.overrides.iter().map(|&(_, ov)| ov),
            (group, name),
            Section::Linter,
            self.config
                .linter
                .resolve_rule(group, name, recommended, domains),
        )
    }
}

fn options_location(
    index: Option<usize>,
    key: &str,
    group: &str,
    rule_name: &str,
) -> Vec<ConfigPathSegment> {
    let mut location = Vec::with_capacity(if index.is_some() { 7 } else { 5 });
    if let Some(index) = index {
        location.push(ConfigPathSegment::Key("overrides".to_owned()));
        location.push(ConfigPathSegment::Index(index));
    }
    location.extend(
        [key, "rules", group, rule_name, "options"]
            .map(|key| ConfigPathSegment::Key(key.to_owned())),
    );
    location
}

impl FalconConfig {
    /// Select the package configuration and match its overrides once per file.
    #[must_use]
    pub fn resolve_file(&self, path: &str) -> ResolvedFileConfig<'_> {
        if self.package_configs.is_empty() && self.overrides.is_empty() {
            return ResolvedFileConfig {
                config: self,
                overrides: Vec::new(),
            };
        }
        let path = self.files.includes.match_path(path);
        let config = self.config_for_identity(&path);
        let overrides = config
            .overrides
            .iter()
            .enumerate()
            .filter(|(_, ov)| ov.includes.admits_identity(&path))
            .collect();
        ResolvedFileConfig { config, overrides }
    }

    /// Resolve a rule's severity, or `None` if it is disabled.
    /// See [`LinterConfig::resolve_rule`].
    #[must_use]
    pub fn resolve_rule(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> Option<ResolvedSeverity> {
        self.linter.resolve_rule(group, name, recommended, domains)
    }

    /// Return effective file-rule options and their parsed cache for `path`.
    ///
    /// Lookup is scoped to the rule's own group, like level resolution.
    /// Matching overrides replace the whole options value in order, not individual
    /// fields. An override with absent or null options leaves earlier options intact,
    /// independently of its level.
    #[must_use]
    pub fn rule_options_data_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Option<&RuleOptions> {
        self.options_data_for(path, group, rule_name, Section::Linter)
            .0
    }

    /// Return effective cross-file-rule options and their parsed cache.
    ///
    /// Matching overrides replace the whole value in order; absent or null
    /// options leave the previous value intact, independently of rule levels.
    #[must_use]
    pub fn cross_file_options_data_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Option<&RuleOptions> {
        self.options_data_for(path, group, rule_name, Section::CrossFile)
            .0
    }

    /// Return the effective file-rule autofix policy for `path`.
    ///
    /// Matching overrides apply in order. An absent fix policy inherits the
    /// previous policy independently of levels and options.
    #[must_use]
    pub fn rule_fix_for(&self, path: &str, group: &str, rule_name: &str) -> Option<RuleFix> {
        self.fix_for(path, group, rule_name, Section::Linter)
    }

    /// Return the effective cross-file-rule autofix policy for `path`.
    ///
    /// Matching overrides apply in order; an absent policy leaves the previous
    /// policy intact. Package configurations replace the workspace configuration.
    #[must_use]
    pub fn cross_file_fix_for(&self, path: &str, group: &str, rule_name: &str) -> Option<RuleFix> {
        self.fix_for(path, group, rule_name, Section::CrossFile)
    }

    fn fix_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
        section: Section,
    ) -> Option<RuleFix> {
        let path = self.files.includes.match_path(path);
        let config = self.config_for_identity(&path);
        let rules = match section {
            Section::Linter => &config.linter.rules,
            Section::CrossFile => &config.cross_file.rules,
        };
        let mut result = rules
            .groups
            .get(group)
            .and_then(|rules| rules.get(rule_name))
            .and_then(RuleConfiguration::fix);
        for ov in config
            .overrides
            .iter()
            .filter(|ov| ov.includes.admits_identity(&path))
        {
            if let Some(fix) = section
                .override_rules(ov)
                .and_then(|block| block.rules.groups.get(group))
                .and_then(|rules| rules.get(rule_name))
                .and_then(RuleConfiguration::fix)
            {
                result = Some(fix);
            }
        }
        result
    }

    /// Return the full configuration path of effective file-rule options.
    #[must_use]
    pub fn rule_options_location_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Vec<ConfigPathSegment> {
        self.options_location_for(path, group, rule_name, Section::Linter, "linter")
    }

    /// Return the full configuration path of effective cross-file-rule options.
    #[must_use]
    pub fn cross_file_options_location_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Vec<ConfigPathSegment> {
        self.options_location_for(path, group, rule_name, Section::CrossFile, "cross-file")
    }

    fn options_location_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
        section: Section,
        key: &str,
    ) -> Vec<ConfigPathSegment> {
        let (_, index) = self.options_data_for(path, group, rule_name, section);
        options_location(index, key, group, rule_name)
    }

    fn options_data_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
        section: Section,
    ) -> (Option<&RuleOptions>, Option<usize>) {
        let path = self.files.includes.match_path(path);
        let config = self.config_for_identity(&path);
        let rules = match section {
            Section::Linter => &config.linter.rules,
            Section::CrossFile => &config.cross_file.rules,
        };
        let mut result = rules
            .groups
            .get(group)
            .and_then(|g| g.get(rule_name))
            .and_then(RuleConfiguration::options_data);
        let mut index = None;
        for (override_index, ov) in config
            .overrides
            .iter()
            .enumerate()
            .filter(|(_, ov)| ov.includes.admits_identity(&path))
        {
            if let Some(options) = section
                .override_rules(ov)
                .and_then(|block| block.rules.groups.get(group))
                .and_then(|g| g.get(rule_name))
                .and_then(RuleConfiguration::options_data)
            {
                result = Some(options);
                index = Some(override_index);
            }
        }
        (result, index)
    }

    /// Resolve file-rule severity, applying matching overrides in order (later wins).
    #[must_use]
    pub fn resolve_rule_for(
        &self,
        path: &str,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> Option<ResolvedSeverity> {
        let path = self.files.includes.match_path(path);
        let config = self.config_for_identity(&path);
        config.resolve_for(
            &path,
            (group, name),
            Section::Linter,
            config
                .linter
                .resolve_rule(group, name, recommended, domains),
        )
    }

    /// Resolve a **cross-file** rule's effective severity for a specific `path`:
    /// the base cross-file resolution ([`CrossFileConfig::resolve_rule`]) patched
    /// by every override whose `includes` match, applied in order (later wins).
    /// Mirrors [`Self::resolve_rule_for`] but reads the override's `cross_file`
    /// block and has no domain dimension. `None` means the rule is disabled here.
    #[must_use]
    pub fn resolve_cross_file_rule_for(
        &self,
        path: &str,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> Option<ResolvedSeverity> {
        let path = self.files.includes.match_path(path);
        let config = self.config_for_identity(&path);
        config.resolve_for(
            &path,
            (group, name),
            Section::CrossFile,
            config.cross_file.resolve_rule(group, name, recommended),
        )
    }

    /// Whether a **cross-file** rule is enabled for any path — the base cross-file
    /// config or any override turns it on. Drives cross-file-rule registration,
    /// mirroring [`Self::is_rule_enabled_anywhere`].
    #[must_use]
    pub fn is_cross_file_rule_enabled_anywhere(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> bool {
        self.enabled_anywhere(
            group,
            name,
            Section::CrossFile,
            self.cross_file.resolve_rule(group, name, recommended),
        ) || self
            .package_configs
            .iter()
            .any(|config| config.is_cross_file_rule_enabled_anywhere(group, name, recommended))
    }

    /// Whether the base config or an override enables this rule for any path.
    #[must_use]
    pub fn is_rule_enabled_anywhere(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> bool {
        self.enabled_anywhere(
            group,
            name,
            Section::Linter,
            self.linter.resolve_rule(group, name, recommended, domains),
        ) || self
            .package_configs
            .iter()
            .any(|config| config.is_rule_enabled_anywhere(group, name, recommended, domains))
    }

    fn section_enabled(&self, section: Section) -> bool {
        match section {
            Section::Linter => self.linter.enabled,
            Section::CrossFile => self.cross_file.enabled,
        }
    }

    fn resolve_for(
        &self,
        path: &MatchPath<'_>,
        (group, name): (&str, &str),
        section: Section,
        result: Option<ResolvedSeverity>,
    ) -> Option<ResolvedSeverity> {
        self.resolve_overrides(
            self.overrides
                .iter()
                .filter(|ov| ov.includes.admits_identity(path)),
            (group, name),
            section,
            result,
        )
    }

    fn resolve_overrides<'a>(
        &self,
        overrides: impl Iterator<Item = &'a Override>,
        (group, name): (&str, &str),
        section: Section,
        mut result: Option<ResolvedSeverity>,
    ) -> Option<ResolvedSeverity> {
        if !self.section_enabled(section) {
            return None;
        }
        for ov in overrides {
            let Some(block) = section.override_rules(ov) else {
                continue;
            };
            if block.enabled == Some(false) {
                result = None;
            } else if let Some(cfg) = block.rules.groups.get(group).and_then(|g| g.get(name)) {
                result = level_to_severity(cfg.level());
            }
        }
        result
    }

    fn enabled_anywhere(
        &self,
        group: &str,
        name: &str,
        section: Section,
        base: Option<ResolvedSeverity>,
    ) -> bool {
        self.section_enabled(section)
            && (base.is_some()
                || self.overrides.iter().any(|ov| {
                    section.override_rules(ov).is_some_and(|block| {
                        block.enabled != Some(false)
                            && block
                                .rules
                                .groups
                                .get(group)
                                .and_then(|g| g.get(name))
                                .is_some_and(|cfg| level_to_severity(cfg.level()).is_some())
                    })
                }))
    }
}

#[derive(Clone, Copy)]
enum Section {
    Linter,
    CrossFile,
}

impl Section {
    fn override_rules(self, ov: &Override) -> Option<&OverrideRules> {
        match self {
            Self::Linter => ov.linter.as_ref(),
            Self::CrossFile => ov.cross_file.as_ref(),
        }
    }
}

impl LinterConfig {
    /// Resolve a rule's effective severity, or `None` if disabled.
    /// 1. `enabled == false` → disabled.
    /// 2. Explicit per-rule level under its group wins (and bypasses domain
    ///    gating): `off` → disabled; `on` → Warn; `info`/`warn`/`error` → that.
    /// 3. Rule with domains: enabled if any of its domains resolves enabled. A
    ///    missing domain key defaults to `Recommended`.
    /// 4. No domains: enabled iff the recommended preset is active.
    #[must_use]
    pub fn resolve_rule(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> Option<ResolvedSeverity> {
        let recommended_on = recommended && self.rules.recommended != Some(false);

        let preset_enabled = || {
            if domains.is_empty() {
                recommended_on
            } else {
                domains.iter().any(|domain| {
                    match self
                        .domains
                        .get(*domain)
                        .copied()
                        .unwrap_or(DomainValue::Recommended)
                    {
                        DomainValue::All => true,
                        DomainValue::Recommended => recommended_on,
                        DomainValue::None => false,
                    }
                })
            }
        };
        self.rules
            .resolve_rule(self.enabled, group, name, preset_enabled)
    }
}

/// One component of a configuration location, preserving literal object keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigPathSegment {
    Key(String),
    Index(usize),
}

/// Render a configuration location for diagnostics, never for navigation.
#[must_use]
pub fn format_config_path(path: &[ConfigPathSegment]) -> String {
    let mut rendered = String::new();
    for segment in path {
        match segment {
            ConfigPathSegment::Key(key) => {
                if !rendered.is_empty() {
                    rendered.push('.');
                }
                rendered.push_str(key);
            }
            ConfigPathSegment::Index(index) => {
                use std::fmt::Write;
                write!(rendered, "[{index}]").expect("writing to a String");
            }
        }
    }
    rendered
}

/// Error message and structured location for configuration loading and parsing.
#[derive(Debug)]
pub struct ConfigError(pub String, pub Vec<ConfigPathSegment>);

impl ConfigError {
    /// Construct an error at the configuration root.
    #[must_use]
    pub fn new(message: String) -> Self {
        Self(message, Vec::new())
    }

    /// Construct an error at its original configuration location.
    #[must_use]
    pub fn at(message: String, path: Vec<ConfigPathSegment>) -> Self {
        Self(message, path)
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "config error: {}", self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Locate a deserialization failure by tracking object keys and array indices.
#[must_use]
pub fn deserialize_error_path<T: DeserializeOwned>(
    value: &Value,
    path: &[ConfigPathSegment],
) -> Vec<ConfigPathSegment> {
    let Err(error) = serde_path_to_error::deserialize::<_, T>(value) else {
        return path.to_owned();
    };
    let mut location = Vec::with_capacity(path.len() + error.path().iter().len());
    location.extend_from_slice(path);
    location.extend(error.path().iter().filter_map(|segment| match segment {
        serde_path_to_error::Segment::Seq { index } => Some(ConfigPathSegment::Index(*index)),
        serde_path_to_error::Segment::Map { key } => Some(ConfigPathSegment::Key(key.clone())),
        serde_path_to_error::Segment::Enum { variant } => {
            Some(ConfigPathSegment::Key(variant.clone()))
        }
        // JSON object keys are always strings; there is no non-string key to navigate.
        serde_path_to_error::Segment::Unknown => None,
    }));
    location
}

/// Canonical top-level keys of a falcon.json.
pub const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
    "$schema",
    "files",
    "linter",
    "cross-file",
    "overrides",
    "max-errors",
    "baseline",
    "error-on-warnings",
];

/// Legacy top-level spellings still accepted, each mapped to its canonical key.
/// Recognized so a pre-rename config keeps loading, but warned on so the user
/// knows to run `falcon migrate`.
pub const LEGACY_TOP_LEVEL_KEYS: &[(&str, &str)] = &[
    ("project", "cross-file"),
    ("cross_file", "cross-file"),
    ("max_errors", "max-errors"),
];

/// A loaded configuration and all warnings produced while loading it.
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: FalconConfig,
    pub warnings: Vec<String>,
}

/// Rule groups accepted by structural configuration parsing.
pub const RULE_GROUPS: &[&str] = &[
    "a11y",
    "complexity",
    "correctness",
    "performance",
    "security",
    "style",
    "suspicious",
];
/// Domain names accepted by structural configuration parsing.
pub const DOMAINS: &[&str] = &[
    "flutter",
    "test",
    "bloc",
    "riverpod",
    "provider",
    "flutter_hooks",
    "equatable",
];

#[derive(Clone, Copy)]
enum ConfigObject {
    Root,
    Files,
    Linter,
    CrossFile,
    Override,
    OverrideRules,
    Rules,
    OverrideRuleEntries,
    RuleGroup,
    Rule,
    Domains,
}

fn validate_keys(
    value: &serde_json::Value,
    path: &[ConfigPathSegment],
    kind: ConfigObject,
    warnings: &mut Vec<String>,
) -> Result<(), ConfigError> {
    use ConfigObject as Object;
    if matches!(kind, ConfigObject::Rule) {
        validate_rule_level(value, path)?;
    }
    if !matches!(kind, ConfigObject::Rule) && !value.is_object() {
        return Err(ConfigError::at(
            format!(
                "invalid configuration `{}`: expected an object",
                format_config_path(path)
            ),
            path.to_owned(),
        ));
    }
    let Some(object) = value.as_object() else {
        return Ok(()); // Serde reports wrong value types.
    };
    for (key, value) in object {
        let mut full_path = Vec::with_capacity(path.len() + 1);
        full_path.extend_from_slice(path);
        full_path.push(ConfigPathSegment::Key(key.clone()));
        let canonical = if matches!(kind, ConfigObject::Root | ConfigObject::Override) {
            LEGACY_TOP_LEVEL_KEYS
                .iter()
                .find(|(legacy, canonical)| {
                    *legacy == key
                        && (matches!(kind, ConfigObject::Root) || *canonical == "cross-file")
                })
                .map_or(key.as_str(), |(_, canonical)| *canonical)
        } else {
            key
        };
        if canonical != key {
            warnings.push(format!(
                "falcon.json uses deprecated key `{}` (`{key}`); \
                 use `{canonical}`; run `falcon migrate` to update",
                format_config_path(&full_path)
            ));
        }
        let child = match (kind, canonical) {
            (Object::Root, "$schema" | "max-errors" | "baseline" | "error-on-warnings")
            | (Object::Files | Object::Override, "includes")
            | (Object::Linter | Object::CrossFile | Object::OverrideRules, "enabled")
            | (Object::Rules, "recommended")
            | (Object::Rule, "level" | "options" | "fix") => None,
            (Object::Root, "files") => Some(Object::Files),
            (Object::Root, "linter") => Some(Object::Linter),
            (Object::Root, "cross-file") => Some(Object::CrossFile),
            (Object::Root, "overrides") => {
                if let Some(entries) = value.as_array() {
                    for (index, entry) in entries.iter().enumerate() {
                        let mut entry_path = full_path.clone();
                        entry_path.push(ConfigPathSegment::Index(index));
                        validate_keys(entry, &entry_path, Object::Override, warnings)?;
                    }
                }
                None
            }
            (Object::Override, "linter" | "cross-file") | (Object::Linter, "domains")
                if value.is_null() =>
            {
                None
            }
            (Object::Override, "linter" | "cross-file") => Some(Object::OverrideRules),
            (Object::Linter | Object::CrossFile, "rules") => Some(Object::Rules),
            (Object::OverrideRules, "rules") => Some(Object::OverrideRuleEntries),
            (Object::Linter, "domains") => Some(Object::Domains),
            (Object::Rules | Object::OverrideRuleEntries, group)
                if RULE_GROUPS.contains(&group) =>
            {
                Some(Object::RuleGroup)
            }
            (Object::RuleGroup, _) => Some(Object::Rule),
            (Object::Domains, domain) if DOMAINS.contains(&domain) => None,
            _ => {
                let description = if matches!(kind, Object::Domains) {
                    "unknown domain"
                } else {
                    "unknown key"
                };
                return Err(ConfigError::at(
                    format!("{description} `{}`", format_config_path(&full_path)),
                    full_path,
                ));
            }
        };
        if let Some(child) = child {
            validate_keys(value, &full_path, child, warnings)?;
        }
    }
    Ok(())
}

fn validate_rule_level(
    value: &serde_json::Value,
    path: &[ConfigPathSegment],
) -> Result<(), ConfigError> {
    let (level, level_path) = if let Some(object) = value.as_object() {
        let level = object.get("level").ok_or_else(|| {
            ConfigError::at(
                format!(
                    "invalid configuration `{}`: missing field `level`",
                    format_config_path(path)
                ),
                path.to_owned(),
            )
        })?;
        let mut level_path = path.to_owned();
        level_path.push(ConfigPathSegment::Key("level".to_owned()));
        (level, level_path)
    } else {
        (value, path.to_owned())
    };
    RulePlainConfiguration::deserialize(level).map_err(|error| {
        ConfigError::at(
            format!(
                "invalid configuration `{}`: {error}",
                format_config_path(&level_path)
            ),
            level_path,
        )
    })?;
    Ok(())
}

/// Load structural configuration and return warnings without printing them.
///
/// Application consumers must use the canonicalizing loader in `falcon_rules`,
/// which additionally validates registered rules and returns all warnings.
///
/// # Errors
///
/// Returns an error for unreadable files, invalid JSON or configuration keys,
/// the legacy flat schema, and a zero `max-errors` value.
pub fn deserialize_config(path: &Path) -> Result<LoadedConfig, ConfigError> {
    let content = read_metadata_text(path)
        .map_err(|e| ConfigError::new(format!("failed to read config file: {e}")))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| ConfigError::new(format!("failed to parse config JSON: {e}")))?;
    deserialize_config_value(&value, path)
}

/// Parse structural configuration from JSON using `path` as its configuration location.
///
/// # Errors
///
/// Returns an error for invalid configuration keys, values, or the legacy flat schema.
pub fn deserialize_config_value(
    value: &serde_json::Value,
    path: &Path,
) -> Result<LoadedConfig, ConfigError> {
    if let Some(obj) = value.as_object()
        && ["rules", "exclude_patterns", "severity_override"]
            .iter()
            .any(|k| obj.contains_key(*k))
    {
        return Err(ConfigError::new(
            "falcon.json uses the legacy flat schema; migrate to the biome-style schema \
             (\"linter.rules\" grouped by category, \"files.includes\"). See docs/configuration.md"
                .to_string(),
        ));
    }

    let mut warnings = Vec::new();
    validate_keys(value, &[], ConfigObject::Root, &mut warnings)?;

    let mut config = FalconConfig::deserialize(value).map_err(|error| {
        let path = deserialize_error_path::<FalconConfig>(value, &[]);
        ConfigError::at(
            format!(
                "invalid configuration `{}`: {error}",
                format_config_path(&path)
            ),
            path,
        )
    })?;

    if config.max_errors == Some(0) {
        return Err(ConfigError::at(
            "`max-errors` must be at least 1; use the CLI --max-errors 0 flag to hide output. \
             Omit it (or use null) for no limit."
                .to_string(),
            vec![ConfigPathSegment::Key(
                if value.get("max-errors").is_some() {
                    "max-errors"
                } else {
                    "max_errors"
                }
                .to_owned(),
            )],
        ));
    }

    let cwd;
    let root = if is_home_config(path) {
        cwd = std::env::current_dir()
            .map_err(|e| ConfigError::new(format!("failed to read current directory: {e}")))?;
        cwd.as_path()
    } else {
        path.parent().unwrap_or(Path::new("."))
    };
    config.resolve_paths(root);
    warnings.extend(config.path_warnings());
    Ok(LoadedConfig { config, warnings })
}

/// Whether `path` names `$HOME/.falcon.json`, resolving directory aliases but not config symlinks.
#[must_use]
pub fn is_home_config(path: &Path) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let home_config = PathBuf::from(home).join(".falcon.json");
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = normalize_path(&cwd.join(path));
    let home_config = normalize_path(&cwd.join(home_config));
    path == home_config
        || (path.file_name() == home_config.file_name()
            && path
                .parent()
                .and_then(|parent| std::fs::canonicalize(parent).ok())
                .zip(
                    home_config
                        .parent()
                        .and_then(|parent| std::fs::canonicalize(parent).ok()),
                )
                .is_some_and(|(path, home)| path == home))
}

/// Find a config file starting from `start_dir`, following this priority order:
///
/// 1. `start_dir/falcon.json` — if exists, return it
/// 2. Walk parent dirs to the nearest `.git` or `.jj` entry and check its `falcon.json`
/// 3. `$HOME/.falcon.json` (via `std::env::var("HOME")`)
/// 4. Return None if nothing found
#[must_use]
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    let local_config = start_dir.join("falcon.json");
    if local_config.exists() {
        return Some(local_config);
    }

    let mut current = start_dir;
    loop {
        if current.join(".git").exists() || current.join(".jj").exists() {
            let config_at_repository_root = current.join("falcon.json");
            if config_at_repository_root.exists() {
                return Some(config_at_repository_root);
            }
            break;
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_config = PathBuf::from(home).join(".falcon.json");
        if home_config.exists() {
            return Some(home_config);
        }
    }

    None
}

#[cfg(test)]
mod path_resolution_tests {
    use super::FalconConfig;
    use std::sync::atomic::Ordering;

    fn config() -> FalconConfig {
        serde_json::from_value(serde_json::json!({
            "files": { "includes": ["lib/**"] },
            "overrides": [
                { "includes": ["lib/**"] },
                { "includes": ["lib/**/*.dart"] }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn walked_directories_do_not_canonicalize() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("lib/nested")).unwrap();
        let mut config = config();
        config.resolve_paths(directory.path());
        let root = config.files.includes.root.as_ref().unwrap();
        for base in [&root.lexical, &root.canonical] {
            for name in ["lib", "lib/nested"] {
                assert!(
                    !config
                        .files
                        .includes
                        .excludes_walked_directory(&base.join(name))
                );
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn walked_file_admission_does_not_repeat_parent_resolution() {
        let directory = tempfile::tempdir().unwrap();
        let package_dir = directory.path().join("packages/example");
        std::fs::create_dir_all(package_dir.join("lib")).unwrap();
        let mut workspace = config();
        workspace.resolve_paths(directory.path());
        let mut package = config();
        package.resolve_paths(&package_dir);
        workspace.set_package_configs(vec![package]);
        let filter = &workspace.files.includes;
        let resolved = filter.resolve_walk_root(&package_dir);
        let probes = || {
            std::iter::once(filter)
                .chain(&filter.package_filters)
                .map(|filter| {
                    filter
                        .root
                        .as_ref()
                        .unwrap()
                        .canonicalizations
                        .load(Ordering::Relaxed)
                })
                .sum::<usize>()
        };
        let before = probes();
        for name in ["lib/first.dart", "lib/second.dart", "lib/unsaved.dart"] {
            assert!(filter.admits_walked_file(&resolved.join(name)));
            assert!(!filter.excludes_walked_file(&resolved.join("lib/pubspec.yaml")));
        }
        assert_eq!(probes(), before);
    }
    #[test]
    fn general_lookup_resolves_once_across_packages_and_overrides() {
        let directory = tempfile::tempdir().unwrap();
        let mut workspace = config();
        workspace.resolve_paths(directory.path());
        let mut packages = Vec::with_capacity(4);
        for index in 0..4 {
            let base = directory.path().join(format!("package{index}"));
            std::fs::create_dir_all(base.join("lib")).unwrap();
            let mut package = config();
            package.resolve_paths(&base);
            packages.push(package);
        }
        workspace.set_package_configs(packages);
        let path = directory.path().join("package0/lib/main.dart");
        let path = path.to_string_lossy();
        let probes = || {
            std::iter::once(&workspace)
                .chain(&workspace.package_configs)
                .map(|config| {
                    config
                        .files
                        .includes
                        .root
                        .as_ref()
                        .unwrap()
                        .canonicalizations
                        .load(Ordering::Relaxed)
                })
                .sum::<usize>()
        };
        let _ = workspace.resolve_rule_for(&path, "style", "test", true, &[]);
        assert_eq!(probes(), 1);
        assert!(std::ptr::eq(
            workspace.config_for_path(&path),
            std::ptr::from_ref(&workspace.package_configs[0]),
        ));
        assert_eq!(probes(), 2);
        assert!(workspace.files.includes.admits(&path));
        assert_eq!(probes(), 3);
        assert!(!workspace.files.includes.excludes(&path));
        assert_eq!(probes(), 4);
        let _ = workspace.rule_options_data_for(&path, "style", "test");
        assert_eq!(probes(), 5);
        let _ = workspace.rule_fix_for(&path, "style", "test");
        assert_eq!(probes(), 6);
        let _ = workspace.resolve_cross_file_rule_for(&path, "style", "test", true);
        assert_eq!(probes(), 7);
        let _ = workspace.cross_file_options_data_for(&path, "style", "test");
        assert_eq!(probes(), 8);
        let _ = workspace.cross_file_fix_for(&path, "style", "test");
        assert_eq!(probes(), 9);
        assert!(
            !workspace
                .files
                .includes
                .excludes_directory(std::path::Path::new(path.as_ref()).parent().unwrap())
        );
        assert_eq!(probes(), 10);
    }

    #[cfg(unix)]
    #[test]
    fn package_lookup_refreshes_replaced_and_retargeted_directory_aliases() {
        let directory = tempfile::tempdir().unwrap();
        let mut workspace = config();
        workspace.resolve_paths(directory.path());
        let packages = ["first", "second"].map(|name| {
            let base = directory.path().join(name);
            std::fs::create_dir_all(base.join("lib")).unwrap();
            let mut package = config();
            package.resolve_paths(&base);
            package
        });
        workspace.set_package_configs(packages.into());
        let cloned = workspace.clone();
        let alias = directory.path().join("alias");
        std::fs::create_dir(&alias).unwrap();
        let path = alias.join("lib/main.dart");
        let path = path.to_string_lossy();
        assert!(std::ptr::eq(
            workspace.config_for_path(&path),
            std::ptr::from_ref(&workspace),
        ));
        assert!(!workspace.files.includes.admits(&path));
        std::fs::remove_dir(&alias).unwrap();
        for (index, name) in ["first", "second"].into_iter().enumerate() {
            std::os::unix::fs::symlink(directory.path().join(name), &alias).unwrap();
            for config in [&workspace, &cloned] {
                assert!(std::ptr::eq(
                    config.config_for_path(&path),
                    std::ptr::from_ref(&config.package_configs[index]),
                ));
                assert!(config.files.includes.admits(&path));
                assert!(config.config_for_path(&path).overrides[0].matches(&path));
            }
            std::fs::remove_file(&alias).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn walked_root_aliases_preserve_internal_and_external_exclusions() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project");
        std::fs::create_dir_all(project.join("lib/generated")).unwrap();
        let outside = directory.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let project_alias = directory.path().join("project-link");
        std::os::unix::fs::symlink(&project, &project_alias).unwrap();
        std::os::unix::fs::symlink(project.join("lib"), project.join("lib-link")).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("external")).unwrap();
        let ancestor_alias = directory.path().join("ancestor-link");
        std::os::unix::fs::symlink(directory.path(), &ancestor_alias).unwrap();
        let alternate_project = ancestor_alias.join("project");
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "files": { "includes": ["**", "!lib/generated", "!external/generated"] }
        }))
        .unwrap();
        config.resolve_paths(&project_alias);
        let filter = &config.files.includes;
        for base in [&project, &project_alias, &alternate_project] {
            for name in ["lib", "lib-link", "external"] {
                let resolved = filter.resolve_walk_root(&base.join(name));
                let root = filter.root.as_ref().unwrap();
                let probes = root.canonicalizations.load(Ordering::Relaxed);
                assert!(!filter.excludes_walked_directory(&resolved));
                assert!(filter.excludes_walked_directory(&resolved.join("generated")));
                assert!(filter.admits_walked_file(&resolved.join("main.dart")));
                assert!(!filter.admits_walked_file(&resolved.join("generated/main.dart")));
                assert!(!filter.excludes_walked_file(&resolved.join("pubspec.yaml")));
                assert!(filter.excludes_walked_file(&resolved.join("generated/pubspec.yaml")));
                assert_eq!(root.canonicalizations.load(Ordering::Relaxed), probes);
            }
        }
    }

    #[test]
    fn walked_package_replacements_keep_ancestors_without_canonicalization() {
        let directory = tempfile::tempdir().unwrap();
        let package_dir = directory.path().join("packages/example");
        std::fs::create_dir_all(package_dir.join("lib/generated")).unwrap();
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "files": { "includes": ["**", "!packages/**"] }
        }))
        .unwrap();
        config.resolve_paths(directory.path());
        let mut package: FalconConfig = serde_json::from_value(serde_json::json!({
            "files": { "includes": ["**", "!lib/generated"] }
        }))
        .unwrap();
        package.resolve_paths(&package_dir);
        config.set_package_configs(vec![package]);
        let filter = &config.files.includes;
        for name in ["packages", "packages/example", "packages/example/lib"] {
            assert!(!filter.excludes_walked_directory(&directory.path().join(name)));
        }
        assert!(filter.excludes_walked_directory(&package_dir.join("lib/generated")));
        for filter in std::iter::once(filter).chain(&filter.package_filters) {
            assert_eq!(
                filter
                    .root
                    .as_ref()
                    .unwrap()
                    .canonicalizations
                    .load(Ordering::Relaxed),
                0
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn replaced_directory_and_missing_parent_refresh_alias_matching() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project");
        std::fs::create_dir_all(project.join("lib")).unwrap();
        let outside = directory.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let alias = project.join("alias");
        std::fs::create_dir(&alias).unwrap();
        let missing = project.join("missing");
        let mut config = config();
        config.resolve_paths(&project);
        let cloned = config.clone();
        for parent in [&alias, &missing] {
            assert!(
                !config
                    .files
                    .includes
                    .admits(&parent.join("unsaved.dart").to_string_lossy())
            );
        }
        std::fs::remove_dir(&alias).unwrap();
        for parent in [&alias, &missing] {
            std::os::unix::fs::symlink(project.join("lib"), parent).unwrap();
            for config in [&config, &cloned] {
                let path = parent.join("unsaved.dart");
                assert!(config.files.includes.admits(&path.to_string_lossy()));
                assert!(config.overrides[0].matches(&path.to_string_lossy()));
            }
            std::fs::remove_file(parent).unwrap();
            std::os::unix::fs::symlink(&outside, parent).unwrap();
            assert!(
                !config
                    .files
                    .includes
                    .admits(&parent.join("unsaved.dart").to_string_lossy())
            );
        }
    }

    #[test]
    fn lexical_parent_resolution_is_fresh_across_files_and_overrides() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = config();
        config.resolve_paths(directory.path());
        let root = config.files.includes.root.as_ref().unwrap();
        for name in ["lib/first.dart", "lib/missing/second.dart"] {
            let path = root.lexical.join(name);
            let path = path.to_string_lossy();
            assert!(config.files.includes.admits(&path));
            for entry in &config.overrides {
                assert!(entry.matches(&path));
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 15);
        for name in ["lib/third.dart", "lib/missing/fourth.dart"] {
            let path = root.lexical.join(name);
            let path = path.to_string_lossy();
            assert!(config.files.includes.admits(&path));
            for entry in &config.overrides {
                assert!(entry.matches(&path));
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 30);
    }

    #[cfg(unix)]
    #[test]
    fn internal_alias_parent_resolution_is_fresh_for_clones_and_overrides() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("lib")).unwrap();
        let alias = directory.path().join("lib-link");
        std::os::unix::fs::symlink(directory.path().join("lib"), &alias).unwrap();
        let mut config = config();
        config.resolve_paths(directory.path());
        let cloned = config.clone();
        let root = config.files.includes.root.as_ref().unwrap();
        for config in [&config, &cloned] {
            for name in [
                "first.dart",
                "second.dart",
                "missing/third.dart",
                "missing/fourth.dart",
            ] {
                let path = alias.join(name);
                let path = path.to_string_lossy();
                assert!(config.files.includes.admits(&path));
                for entry in &config.overrides {
                    assert!(entry.matches(&path));
                }
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 36);
    }

    #[cfg(windows)]
    #[test]
    fn windows_prefix_spellings_preserve_internal_links_with_fresh_parent_probes() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project");
        let target = directory.path().join("outside");
        std::fs::create_dir_all(project.join("lib")).unwrap();
        std::fs::create_dir(&target).unwrap();
        // Directory junctions do not require symlink privileges or Developer Mode.
        let junction = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(project.join("lib/plugin"))
            .arg(&target)
            .output()
            .unwrap();
        assert!(junction.status.success(), "{junction:?}");
        let canonical = std::fs::canonicalize(&project).unwrap();
        let ordinary =
            std::path::Path::new(canonical.to_str().unwrap().strip_prefix(r"\\?\").unwrap());
        let mut config = config();
        config.resolve_paths(&canonical);
        let root = config.files.includes.root.as_ref().unwrap();
        for base in [ordinary, canonical.as_path()] {
            let path = base.join("lib/plugin/unsaved.dart");
            let path = path.to_string_lossy();
            assert!(config.files.includes.admits(&path));
            for entry in &config.overrides {
                assert!(entry.matches(&path));
            }
        }
        let probes = root.canonicalizations.load(Ordering::Relaxed);
        assert_eq!(probes, 6);
        for base in [ordinary, canonical.as_path()] {
            let path = base.join("lib/plugin/another.dart");
            assert!(config.files.includes.admits(&path.to_string_lossy()));
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), probes + 2);
    }

    #[test]
    fn outside_parent_resolution_is_fresh_and_stops_at_first_existing_parent() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project");
        let outside = directory.path().join("outside");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let mut config = config();
        config.resolve_paths(&project);
        let root = config.files.includes.root.as_ref().unwrap();
        for name in ["first.dart", "second.dart"] {
            let path = outside.join(name);
            let path = path.to_string_lossy();
            assert!(!config.files.includes.admits(&path));
            for entry in &config.overrides {
                assert!(!entry.matches(&path));
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 6);

        // Each lookup resolves only as far as the nearest existing ancestor.
        for name in ["first.dart", "second.dart"] {
            let path = outside.join("missing/deeper").join(name);
            let path = path.to_string_lossy();
            assert!(!config.files.includes.admits(&path));
            for entry in &config.overrides {
                assert!(!entry.matches(&path));
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 24);
    }
}

#[cfg(test)]
mod warning_tests {
    use super::{ConfigObject, validate_keys};
    use serde_json::json;

    #[test]
    fn unknown_keys_report_full_paths_at_every_structural_depth() {
        for (value, path) in [
            (json!({ "linterr": {} }), "linterr"),
            (json!({ "files": { "include": [] } }), "files.include"),
            (json!({ "linter": { "enable": false } }), "linter.enable"),
            (
                json!({ "cross-file": { "enable": false } }),
                "cross-file.enable",
            ),
            (
                json!({ "linter": { "rules": { "stile": {} } } }),
                "linter.rules.stile",
            ),
            (
                json!({ "linter": { "domains": { "fluter": "all" } } }),
                "linter.domains.fluter",
            ),
            (
                json!({ "overrides": [{ "include": [] }] }),
                "overrides[0].include",
            ),
            (
                json!({ "overrides": [{ "linter": { "domains": {} } }] }),
                "overrides[0].linter.domains",
            ),
            (
                json!({ "overrides": [{ "cross-file": { "rules": { "stile": {} } } }] }),
                "overrides[0].cross-file.rules.stile",
            ),
            (
                json!({ "overrides": [{ "linter": { "rules": { "style": { "x": { "level": "warn", "option": {} } } } } }] }),
                "overrides[0].linter.rules.style.x.option",
            ),
        ] {
            let error = validate_keys(&value, &[], ConfigObject::Root, &mut Vec::new())
                .expect_err("unknown keys must fail");
            assert!(error.0.contains(path), "{error}: expected {path}");
        }
    }

    #[test]
    fn legacy_keys_return_warnings_and_rule_options_remain_opaque() {
        let value = json!({
            "project": {}, "max_errors": 10,
            "linter": { "rules": { "style": {
                "x": { "level": "warn", "options": { "rule-specific": {} } }
            } } },
            "overrides": [{ "cross_file": {} }]
        });
        let mut warnings = Vec::new();
        validate_keys(&value, &[], ConfigObject::Root, &mut warnings).unwrap();
        assert_eq!(warnings.len(), 3);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("overrides[0].cross_file"))
        );
        assert!(warnings.iter().all(|warning| warning.contains("deprecated")
            && warning.contains("falcon migrate")));
    }
    #[test]
    fn structural_config_typos_fail_without_falling_back_to_defaults() {
        let directory =
            std::env::temp_dir().join(format!("falcon-strict-config-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("falcon.json");
        for (value, key_path) in [
            (
                json!({ "overrides": [{ "include": ["test/**"] }] }),
                "overrides[0].include",
            ),
            (
                json!({ "linter": { "domains": { "fluter": "recommended" } } }),
                "linter.domains.fluter",
            ),
        ] {
            std::fs::write(&path, value.to_string()).unwrap();
            let error = super::deserialize_config(&path).unwrap_err();
            assert!(error.0.contains(key_path), "{error}");
        }
        std::fs::write(
            &path,
            json!({
                "overrides": [{ "linter": { "enabled": false } }]
            })
            .to_string(),
        )
        .unwrap();
        let loaded = super::deserialize_config(&path).unwrap();
        assert!(loaded.config.overrides[0].matches("anywhere.dart"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_structural_values_identify_the_exact_path() {
        for (value, path) in [
            (json!({"linter": {"enabled": "bad"}}), "linter.enabled"),
            (
                json!({"files": {"includes": ["lib/**", 4]}}),
                "files.includes[1]",
            ),
            (
                json!({"overrides": [{"linter": {"enabled": []}}]}),
                "overrides[0].linter.enabled",
            ),
        ] {
            let error =
                super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
                    .unwrap_err();
            assert!(
                error
                    .0
                    .starts_with(&format!("invalid configuration `{path}`:")),
                "{error}"
            );
        }
    }

    #[test]
    fn invalid_rule_levels_identify_the_entry_before_flattened_deserialization() {
        for (entry, suffix) in [
            (json!("warning"), ""),
            (json!({"options": {}}), ""),
            (json!(true), ""),
            (json!({"level": "warning"}), ".level"),
        ] {
            for overridden in [false, true] {
                let mut value = json!({
                    "linter": {"rules": {
                        "complexity": {"max-lines-for-file": entry},
                        "style": {"prefer-first": "warn"}
                    }}
                });
                let prefix = if overridden {
                    value = json!({"overrides": [value]});
                    "overrides[0]."
                } else {
                    ""
                };
                let error =
                    super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
                        .unwrap_err();
                let path = format!("{prefix}linter.rules.complexity.max-lines-for-file{suffix}");
                assert_eq!(super::format_config_path(&error.1), path);
                assert!(
                    error
                        .0
                        .starts_with(&format!("invalid configuration `{path}`:")),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn null_optional_fields_are_absent() {
        let value = json!({
            "$schema": null,
            "max-errors": null,
            "linter": {"rules": {"recommended": null}, "domains": null},
            "cross-file": {"rules": {"recommended": null}},
            "overrides": [
                {"linter": null, "cross-file": null},
                {"project": null},
                {"cross_file": null},
                {"linter": {"enabled": null}, "cross-file": {"enabled": null}}
            ]
        });
        let loaded =
            super::deserialize_config_value(&value, std::path::Path::new("falcon.json")).unwrap();
        assert!(loaded.config.schema.is_none());
        assert!(loaded.config.max_errors.is_none());
        assert!(loaded.config.linter.rules.recommended.is_none());
        assert!(loaded.config.linter.domains.is_empty());
        assert!(loaded.config.cross_file.rules.recommended.is_none());
        for entry in &loaded.config.overrides[..3] {
            assert!(entry.linter.is_none());
            assert!(entry.cross_file.is_none());
        }
        let entry = &loaded.config.overrides[3];
        assert!(entry.linter.as_ref().unwrap().enabled.is_none());
        assert!(entry.cross_file.as_ref().unwrap().enabled.is_none());
        assert_eq!(loaded.warnings.len(), 2);
    }

    #[test]
    fn null_required_sections_still_fail_at_the_section() {
        use super::ConfigPathSegment::Key;
        for key in ["files", "linter", "cross-file", "overrides"] {
            let value = json!({key: null});
            let error =
                super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
                    .unwrap_err();
            assert_eq!(error.1, vec![Key(key.to_owned())]);
        }
    }

    #[test]
    fn structured_paths_preserve_backticks_dots_and_literal_indices() {
        use super::ConfigPathSegment::{Index, Key};
        let value = json!({"overrides": [
            {},
            {"linter": {"rules": {"style": {
                "rule`.[2]": {"level": "warn", "unknown`.[4]": true}
            }}}}
        ]});
        let error = super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
            .unwrap_err();
        assert_eq!(
            error.1,
            vec![
                Key("overrides".to_owned()),
                Index(1),
                Key("linter".to_owned()),
                Key("rules".to_owned()),
                Key("style".to_owned()),
                Key("rule`.[2]".to_owned()),
                Key("unknown`.[4]".to_owned()),
            ]
        );
    }

    #[test]
    fn deserialization_paths_preserve_literal_keys_and_nested_array_indices() {
        use super::ConfigPathSegment::{Index, Key};
        type Options = std::collections::BTreeMap<String, Vec<Vec<bool>>>;
        let value = json!({"option`.[9]": [[true], [false, "invalid"]]});
        let path = super::deserialize_error_path::<Options>(&value, &[Key("options".to_owned())]);
        assert_eq!(
            path,
            vec![
                Key("options".to_owned()),
                Key("option`.[9]".to_owned()),
                Index(1),
                Index(1),
            ]
        );
    }

    #[test]
    fn missing_fields_are_located_at_the_containing_object() {
        #[derive(serde::Deserialize)]
        struct Required {
            #[serde(rename = "required")]
            _required: bool,
        }
        type Options = std::collections::BTreeMap<String, Required>;
        let value = json!({"option`.[9]": {}});
        assert_eq!(
            super::deserialize_error_path::<Options>(&value, &[]),
            vec![super::ConfigPathSegment::Key("option`.[9]".to_owned())],
        );
    }

    #[test]
    fn container_type_errors_stay_at_the_field() {
        use super::ConfigPathSegment::Key;
        type Options = std::collections::BTreeMap<String, bool>;
        for invalid in [json!([]), json!({}), json!([true])] {
            let value = json!({"option`.[9]": invalid});
            assert_eq!(
                super::deserialize_error_path::<Options>(&value, &[]),
                vec![Key("option`.[9]".to_owned())],
            );
        }
    }

    #[test]
    fn custom_validation_errors_stay_at_the_last_field() {
        #[derive(serde::Deserialize)]
        struct Checked {
            #[serde(rename = "limit", deserialize_with = "reject")]
            _limit: u8,
        }
        fn reject<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
            let _ = <u8 as serde::Deserialize>::deserialize(deserializer)?;
            Err(serde::de::Error::custom("rejected after deserialization"))
        }
        type Options = std::collections::BTreeMap<String, Checked>;
        let value = json!({"option`.[9]": {"limit": 1}});
        assert_eq!(
            super::deserialize_error_path::<Options>(&value, &[]),
            vec![
                super::ConfigPathSegment::Key("option`.[9]".to_owned()),
                super::ConfigPathSegment::Key("limit".to_owned()),
            ],
        );
    }

    #[test]
    fn scalar_errors_include_structured_array_paths() {
        use super::ConfigPathSegment::{Index, Key};
        let value = json!({"files": {"includes": ["lib/**", 4]}});
        let error = super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
            .unwrap_err();
        assert_eq!(
            error.1,
            vec![
                Key("files".to_owned()),
                Key("includes".to_owned()),
                Index(1)
            ],
        );
    }

    #[test]
    fn zero_max_errors_preserves_the_input_key_spelling() {
        for key in ["max-errors", "max_errors"] {
            let value = json!({key: 0});
            let error =
                super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
                    .unwrap_err();
            assert_eq!(error.1, vec![super::ConfigPathSegment::Key(key.to_owned())],);
        }
    }
}

#[cfg(test)]
mod option_tests {
    use super::{FalconConfig, RuleConfiguration, RuleFix, RuleOptions};
    use serde_json::json;

    #[test]
    fn failed_initialization_is_cached_until_the_value_changes() {
        let mut options: RuleOptions = serde_json::from_value(json!({ "limit": "bad" })).unwrap();
        assert_eq!(
            options.get_or_try_init::<u64, _>(|_| Err("invalid options")),
            Err(&"invalid options"),
        );
        assert_eq!(
            options.get_or_try_init::<u64, &str>(|_| panic!("must reuse validation error")),
            Err(&"invalid options"),
        );
        options.value_mut()["limit"] = json!(4);
        assert_eq!(
            options.get_or_try_init::<u64, &str>(|value| Ok(value["limit"].as_u64().unwrap())),
            Ok(&4),
        );
    }

    #[test]
    fn parsed_options_are_cached_shared_on_clone_and_invalidated_on_mutation() {
        let value = json!({ "limit": 4 });
        let options: RuleOptions = serde_json::from_value(value.clone()).unwrap();
        let parsed = options.get_or_init(|value| value["limit"].as_u64().unwrap());
        assert_eq!(*parsed, 4);
        assert!(std::ptr::eq(
            parsed,
            options.get_or_init::<u64>(|_| panic!("must reuse cached data")),
        ));

        let mut cloned = options.clone();
        assert!(std::ptr::eq(
            parsed,
            cloned.get_or_init::<u64>(|_| panic!("must share cached data")),
        ));
        cloned.value_mut()["limit"] = json!(7);
        assert_eq!(
            *cloned.get_or_init(|value| value["limit"].as_u64().unwrap()),
            7,
        );
        assert_eq!(*parsed, 4);
        assert_eq!(options.value(), &value);
        assert_eq!(serde_json::to_value(&options).unwrap(), value);

        let decoded: RuleOptions = serde_json::from_value(value).unwrap();
        assert_eq!(
            *decoded.get_or_init(|_| String::from("fresh cache")),
            "fresh cache",
        );
    }

    #[test]
    fn rule_configuration_keeps_its_json_shape_and_absent_options() {
        for value in [
            json!("warn"),
            json!({ "level": "warn", "options": null }),
            json!({ "level": "warn", "options": { "limit": 4 } }),
        ] {
            let config: RuleConfiguration = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(&config).unwrap(), value);
        }
        let config: RuleConfiguration = serde_json::from_value(json!({ "level": "warn" })).unwrap();
        assert!(config.options_data().is_none());
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            json!({ "level": "warn", "options": null }),
        );
    }

    #[test]
    fn option_selectors_replace_values_and_ignore_levels_nulls_and_other_sections() {
        for section in ["linter", "cross-file"] {
            let config: FalconConfig = serde_json::from_value(json!({
                section: {
                    "rules": { "style": {
                        "example": { "level": "warn", "options": { "base": true, "limit": 4 } }
                    } }
                },
                "overrides": [
                    {
                        "includes": ["test/**"],
                        section: { "rules": { "style": {
                            "example": { "level": "warn", "options": { "limit": 2 } }
                        } } }
                    },
                    {
                        "includes": ["test/**"],
                        section: { "rules": { "style": { "example": "off" } } }
                    },
                    {
                        "includes": ["test/**"],
                        section: { "rules": { "style": {
                            "example": { "level": "error", "options": null }
                        } } }
                    },
                    {
                        "includes": ["test/special/**"],
                        section: { "rules": { "style": {
                            "example": { "level": "warn", "options": {} }
                        } } }
                    }
                ]
            }))
            .unwrap();
            for (path, expected) in [
                ("lib/main.dart", json!({ "base": true, "limit": 4 })),
                ("test/main.dart", json!({ "limit": 2 })),
                ("test/special/main.dart", json!({})),
            ] {
                let file = config.rule_options_data_for(path, "style", "example");
                let cross_file = config.cross_file_options_data_for(path, "style", "example");
                let (selected, other) = if section == "linter" {
                    (file, cross_file)
                } else {
                    (cross_file, file)
                };
                assert_eq!(selected.map(RuleOptions::value), Some(&expected));
                assert!(other.is_none());
            }
            assert!(
                config
                    .rule_options_data_for("test/main.dart", "wrong-group", "example")
                    .is_none()
            );
            assert!(
                config
                    .cross_file_options_data_for("test/main.dart", "wrong-group", "example")
                    .is_none()
            );
        }
    }

    #[test]
    fn fix_policies_parse_serialize_and_validate_at_every_section() {
        for (name, expected) in [
            ("none", RuleFix::None),
            ("safe", RuleFix::Safe),
            ("unsafe", RuleFix::Unsafe),
        ] {
            for options in [serde_json::Value::Null, json!({ "limit": 4 })] {
                let value = json!({ "level": "warn", "fix": name, "options": options });
                let configuration: RuleConfiguration =
                    serde_json::from_value(value.clone()).unwrap();
                assert_eq!(configuration.fix(), Some(expected));
                assert_eq!(serde_json::to_value(configuration).unwrap(), value);
            }
            for section in ["linter", "cross-file"] {
                let value = json!({
                    section: { "rules": { "style": {
                        "example": { "level": "warn", "fix": name }
                    } } },
                    "overrides": [{
                        section: { "rules": { "style": {
                            "example": { "level": "error", "fix": name }
                        } } }
                    }]
                });
                let loaded =
                    super::deserialize_config_value(&value, std::path::Path::new("falcon.json"))
                        .unwrap();
                let selected = if section == "linter" {
                    loaded
                        .config
                        .rule_fix_for("lib/main.dart", "style", "example")
                } else {
                    loaded
                        .config
                        .cross_file_fix_for("lib/main.dart", "style", "example")
                };
                assert_eq!(selected, Some(expected));
            }
        }
        for value in [json!("invalid"), json!(true), json!(1), json!(null)] {
            assert!(
                serde_json::from_value::<RuleConfiguration>(
                    json!({ "level": "warn", "fix": value })
                )
                .is_err()
            );
        }
        let plain: RuleConfiguration = serde_json::from_value(json!("warn")).unwrap();
        let object: RuleConfiguration = serde_json::from_value(json!({ "level": "warn" })).unwrap();
        assert_eq!(plain.fix(), None);
        assert_eq!(object.fix(), None);
        assert!(serde_json::to_value(object).unwrap().get("fix").is_none());
    }

    #[test]
    fn fix_selectors_inherit_and_apply_last_explicit_matching_override() {
        for section in ["linter", "cross-file"] {
            let other_section = if section == "linter" {
                "cross-file"
            } else {
                "linter"
            };
            let config: FalconConfig = serde_json::from_value(json!({
                section: { "rules": { "style": {
                    "example": { "level": "warn", "fix": "safe", "options": {} }
                } } },
                "overrides": [
                    {
                        "includes": ["test/**"],
                        section: { "rules": { "style": {
                            "example": { "level": "warn", "fix": "none" }
                        } } }
                    },
                    {
                        "includes": ["test/**"],
                        section: { "rules": { "style": { "example": "off" } } }
                    },
                    {
                        "includes": ["test/**"],
                        section: { "rules": { "style": {
                            "example": { "level": "error", "options": {} }
                        } } },
                        other_section: { "rules": { "style": {
                            "example": { "level": "warn", "fix": "safe" }
                        } } }
                    },
                    {
                        "includes": ["test/special/**"],
                        section: { "rules": { "style": {
                            "example": { "level": "warn", "fix": "unsafe" }
                        } } }
                    }
                ]
            }))
            .unwrap();
            for (path, expected) in [
                ("lib/main.dart", RuleFix::Safe),
                ("test/main.dart", RuleFix::None),
                ("test/special/main.dart", RuleFix::Unsafe),
            ] {
                let selected = if section == "linter" {
                    config.rule_fix_for(path, "style", "example")
                } else {
                    config.cross_file_fix_for(path, "style", "example")
                };
                assert_eq!(selected, Some(expected));
            }
            assert_eq!(
                config.rule_fix_for("test/main.dart", "wrong", "example"),
                None
            );
            assert_eq!(
                config.cross_file_fix_for("test/main.dart", "wrong", "example"),
                None
            );
            assert_eq!(
                config.rule_fix_for("lib/main.dart", "style", "missing"),
                None
            );
            assert_eq!(
                config.cross_file_fix_for("lib/main.dart", "style", "missing"),
                None
            );
        }
    }

    #[test]
    fn fix_selectors_use_nearest_package_replacement_and_package_overrides() {
        for section in ["linter", "cross-file"] {
            let root = std::env::current_dir()
                .unwrap()
                .join("fix-policy-workspace");
            let mut config: FalconConfig = serde_json::from_value(json!({
                section: { "rules": { "style": {
                    "example": { "level": "warn", "fix": "unsafe" }
                } } }
            }))
            .unwrap();
            config.resolve_paths(&root);
            let mut package: FalconConfig = serde_json::from_value(json!({
                section: { "rules": { "style": {
                    "example": { "level": "warn", "fix": "none" }
                } } },
                "overrides": [{
                    "includes": ["test/**"],
                    section: { "rules": { "style": {
                        "example": { "level": "warn", "fix": "safe" }
                    } } }
                }]
            }))
            .unwrap();
            package.resolve_paths(&root.join("packages/app"));
            let mut nested = FalconConfig::default();
            nested.resolve_paths(&root.join("packages/app/nested"));
            config.set_package_configs(vec![package, nested]);
            for (relative, expected) in [
                ("lib/main.dart", Some(RuleFix::Unsafe)),
                ("packages/app/lib/main.dart", Some(RuleFix::None)),
                ("packages/app/test/main.dart", Some(RuleFix::Safe)),
                ("packages/app/nested/main.dart", None),
            ] {
                let path = root.join(relative);
                let path = path.to_str().unwrap();
                let selected = if section == "linter" {
                    config.rule_fix_for(path, "style", "example")
                } else {
                    config.cross_file_fix_for(path, "style", "example")
                };
                assert_eq!(selected, expected, "{relative}");
            }
        }
    }
}
