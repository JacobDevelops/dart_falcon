//! falcon.json configuration schema and loader (biome 2.x-shaped).
//!
//! `FalconConfig` is the contract; every field and its default is documented
//! here. File rules are grouped by category under `linter.rules`; enablement is
//! resolved from an explicit per-rule level, the `recommended` preset, and
//! per-domain gating (see [`LinterConfig::resolve_rule`]). Cross-file rules are a
//! separate feature under `cross_file.rules`, resolved the same way minus domains
//! (see [`CrossFileConfig::resolve_rule`]).

use glob::Pattern;
use serde::de::{self, DeserializeOwned, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

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
    // The exclusive bound avoids accepting 2^64 when usize::MAX rounds up as f64.
    if value < 0.0 || value >= usize::MAX as f64 + 1.0 || value.fract() != 0.0 {
        return Err(range_error());
    }
    Ok(value as usize)
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
}

#[derive(Debug)]
struct PathRoot {
    canonical: PathBuf,
    lexical: PathBuf,
    working_dir: PathBuf,
    // Shared by files.includes and every override, including cloned configs.
    parents: Mutex<HashMap<PathBuf, Option<PathBuf>>>,
    #[cfg(test)]
    canonicalizations: std::sync::atomic::AtomicUsize,
}

impl PathRoot {
    fn relative_parent(&self, parent: &Path) -> Option<PathBuf> {
        let mut resolved = None;
        let mut existing = parent;
        for ancestor in parent.ancestors() {
            existing = ancestor;
            if let Some(cached) = self.parents.lock().unwrap().get(ancestor) {
                if ancestor == parent {
                    return strip_root(cached.as_ref()?, &self.canonical).map(Path::to_path_buf);
                }
                resolved = cached.clone();
                break;
            }
            #[cfg(test)]
            self.canonicalizations
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(canonical) = std::fs::canonicalize(ancestor) {
                resolved = Some(canonical);
                break;
            }
        }
        // Preserve missing descendants without probing them again for another
        // document, override, option lookup, or diagnostic in this directory.
        let mut parents = self.parents.lock().unwrap();
        for ancestor in parent.ancestors() {
            let canonical = resolved
                .as_ref()
                .map(|base| base.join(ancestor.strip_prefix(existing).unwrap()));
            parents.insert(ancestor.to_path_buf(), canonical);
            if ancestor == existing {
                break;
            }
        }
        strip_root(parents.get(parent)?.as_ref()?, &self.canonical).map(Path::to_path_buf)
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
    pub fn from_includes(patterns: &[String]) -> Self {
        patterns.to_vec().into()
    }

    pub fn extend_excludes(&mut self, patterns: &[String]) {
        self.patterns
            .extend(patterns.iter().map(|raw| format!("!{raw}")));
        self.compile();
    }

    pub fn set_base_dir(&mut self, base: &Path) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let lexical = normalize_path(&cwd.join(base));
        self.set_root(Arc::new(PathRoot {
            canonical: std::fs::canonicalize(&lexical).unwrap_or_else(|_| lexical.clone()),
            lexical,
            working_dir: cwd,
            parents: Mutex::default(),
            #[cfg(test)]
            canonicalizations: std::sync::atomic::AtomicUsize::new(0),
        }));
    }

    fn set_root(&mut self, root: Arc<PathRoot>) {
        self.root = Some(root);
        self.compile();
    }

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
        if let Some(suffix) =
            strip_root(path, &root.lexical).or_else(|| strip_root(path, &root.canonical))
        {
            return Some(suffix.to_path_buf());
        }
        // Canonicalize only up to the nearest existing parent. Both sides of
        // strip_prefix then have the same canonical Windows verbatim prefix.
        let parent = path.parent()?;
        let mut suffix = root.relative_parent(parent)?;
        suffix.push(path.file_name()?);
        Some(suffix)
    }

    fn matching_path<'a>(&self, path: &'a str) -> std::borrow::Cow<'a, str> {
        let Some(root) = &self.root else {
            return path.into();
        };
        let input = Path::new(path);
        let normalized = if !input.is_absolute() {
            std::borrow::Cow::Owned(normalize_path(&root.working_dir.join(input)))
        } else if path.contains("//")
            || path.contains("/./")
            || path.contains("/../")
            || path.ends_with("/.")
            || path.ends_with("/..")
            || path.ends_with('/')
        {
            std::borrow::Cow::Owned(normalize_path(input))
        } else {
            std::borrow::Cow::Borrowed(input)
        };
        if !normalized.starts_with(&root.lexical)
            && let Some(suffix) = self.relative_to_base(&normalized)
        {
            return root
                .lexical
                .join(suffix)
                .to_string_lossy()
                .into_owned()
                .into();
        }
        match normalized {
            std::borrow::Cow::Borrowed(_) => path.into(),
            std::borrow::Cow::Owned(path) => path.to_string_lossy().into_owned().into(),
        }
    }

    pub fn excludes(&self, path: &str) -> bool {
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

    pub fn excludes_directory(&self, path: &Path) -> bool {
        let mut path = if let Some(root) = &self.root {
            let normalized = normalize_path(&root.working_dir.join(path));
            let resolved = if normalized.starts_with(&root.lexical) {
                normalized
            } else if let Some(suffix) = strip_root(&normalized, &root.lexical)
                .or_else(|| strip_root(&normalized, &root.canonical))
            {
                if suffix.as_os_str().is_empty() {
                    root.lexical.clone()
                } else {
                    root.lexical.join(suffix)
                }
            } else if let Some(suffix) = root.relative_parent(&normalized) {
                // A directory can itself be a root alias. Resolve it rather
                // than treating its final component as a document filename.
                if suffix.as_os_str().is_empty() {
                    root.lexical.clone()
                } else {
                    root.lexical.join(suffix)
                }
            } else {
                normalized
            };
            resolved.to_string_lossy().into_owned()
        } else {
            path.to_string_lossy().into_owned()
        };
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

    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    pub fn admits(&self, path: &str) -> bool {
        let path = self.matching_path(path);
        !self.excludes_resolved(&path)
            && (self.include.is_empty()
                || self
                    .include
                    .iter()
                    .any(|pattern| matches_path(pattern, &path)))
    }
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
    pub fn root_dir(&self) -> Option<&Path> {
        self.files
            .includes
            .root
            .as_ref()
            .map(|root| root.canonical.as_path())
    }

    pub fn path_warnings(&self) -> Vec<String> {
        let mut warnings = self.files.includes.warnings.clone();
        for entry in &self.overrides {
            warnings.extend_from_slice(entry.includes.warnings());
        }
        warnings
    }

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
    pub fn get_or_try_init<T: Any + Send + Sync, E: Any + Send + Sync>(
        &self,
        initializer: impl FnOnce(&serde_json::Value) -> Result<T, E>,
    ) -> Result<&T, &E> {
        self.get_or_init(initializer).as_ref()
    }
}

/// A single rule's configuration: either a bare level or a level with options.
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
    },
}

impl RuleConfiguration {
    /// The configured level, regardless of form.
    pub fn level(&self) -> RulePlainConfiguration {
        match self {
            RuleConfiguration::Plain(level) => *level,
            RuleConfiguration::WithOptions { level, .. } => *level,
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
                f.write_str("a level string or a { level, options } object")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(RuleConfiguration::Plain(level_from_str(value)?))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut level: Option<RulePlainConfiguration> = None;
                let mut options: Option<RuleOptions> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "level" => {
                            let raw = map.next_value::<String>()?;
                            level = Some(level_from_str(&raw)?);
                        }
                        "options" => options = Some(map.next_value()?),
                        _ => {
                            map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }
                let level = level.ok_or_else(|| de::Error::missing_field("level"))?;
                Ok(RuleConfiguration::WithOptions {
                    level,
                    options: options.unwrap_or_default(),
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

impl FalconConfig {
    /// Resolve a rule's severity, or `None` if it is disabled.
    /// See [`LinterConfig::resolve_rule`].
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
    pub fn cross_file_options_data_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Option<&RuleOptions> {
        self.options_data_for(path, group, rule_name, Section::CrossFile)
            .0
    }

    /// Return the full configuration path of effective file-rule options.
    pub fn rule_options_location_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Vec<ConfigPathSegment> {
        self.options_location_for(path, group, rule_name, Section::Linter, "linter")
    }

    /// Return the full configuration path of effective cross-file-rule options.
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

    fn options_data_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
        section: Section,
    ) -> (Option<&RuleOptions>, Option<usize>) {
        let rules = match section {
            Section::Linter => &self.linter.rules,
            Section::CrossFile => &self.cross_file.rules,
        };
        let mut result = rules
            .groups
            .get(group)
            .and_then(|g| g.get(rule_name))
            .and_then(RuleConfiguration::options_data);
        let mut index = None;
        for (override_index, ov) in self
            .overrides
            .iter()
            .enumerate()
            .filter(|(_, ov)| ov.matches(path))
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
    pub fn resolve_rule_for(
        &self,
        path: &str,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> Option<ResolvedSeverity> {
        self.resolve_for(
            path,
            (group, name),
            Section::Linter,
            self.linter.resolve_rule(group, name, recommended, domains),
        )
    }

    /// Resolve a **cross-file** rule's effective severity for a specific `path`:
    /// the base cross-file resolution ([`CrossFileConfig::resolve_rule`]) patched
    /// by every override whose `includes` match, applied in order (later wins).
    /// Mirrors [`Self::resolve_rule_for`] but reads the override's `cross_file`
    /// block and has no domain dimension. `None` means the rule is disabled here.
    pub fn resolve_cross_file_rule_for(
        &self,
        path: &str,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> Option<ResolvedSeverity> {
        self.resolve_for(
            path,
            (group, name),
            Section::CrossFile,
            self.cross_file.resolve_rule(group, name, recommended),
        )
    }

    /// Whether a **cross-file** rule is enabled for any path — the base cross-file
    /// config or any override turns it on. Drives cross-file-rule registration,
    /// mirroring [`Self::is_rule_enabled_anywhere`].
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
        )
    }

    /// Whether the base config or an override enables this rule for any path.
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
        )
    }

    fn section_enabled(&self, section: Section) -> bool {
        match section {
            Section::Linter => self.linter.enabled,
            Section::CrossFile => self.cross_file.enabled,
        }
    }

    fn resolve_for(
        &self,
        path: &str,
        (group, name): (&str, &str),
        section: Section,
        mut result: Option<ResolvedSeverity>,
    ) -> Option<ResolvedSeverity> {
        if !self.section_enabled(section) {
            return None;
        }
        for ov in self.overrides.iter().filter(|ov| ov.matches(path)) {
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
    pub fn new(message: String) -> Self {
        Self(message, Vec::new())
    }

    /// Construct an error at its original configuration location.
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
    if matches!(kind, ConfigObject::Rule) {
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
                .map(|(_, canonical)| *canonical)
                .unwrap_or(key)
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
        use ConfigObject as Object;
        let child = match (kind, canonical) {
            (Object::Root, "$schema" | "max-errors" | "baseline" | "error-on-warnings")
            | (Object::Files | Object::Override, "includes")
            | (Object::Linter | Object::CrossFile | Object::OverrideRules, "enabled")
            | (Object::Rules, "recommended")
            | (Object::Rule, "level" | "options") => None,
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
            (Object::Override, "linter" | "cross-file") if value.is_null() => None,
            (Object::Override, "linter" | "cross-file") => Some(Object::OverrideRules),
            (Object::Linter | Object::CrossFile, "rules") => Some(Object::Rules),
            (Object::OverrideRules, "rules") => Some(Object::OverrideRuleEntries),
            (Object::Linter, "domains") if value.is_null() => None,
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
    let content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::new(format!("failed to read config file: {}", e)))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| ConfigError::new(format!("failed to parse config JSON: {}", e)))?;
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
/// 2. Walk parent dirs up to filesystem root looking for `.git` dir; when found, check `<git_root>/falcon.json`
/// 3. `$HOME/.falcon.json` (via `std::env::var("HOME")`)
/// 4. Return None if nothing found
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    let local_config = start_dir.join("falcon.json");
    if local_config.exists() {
        return Some(local_config);
    }

    let mut current = start_dir;
    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            let config_at_git_root = current.join("falcon.json");
            if config_at_git_root.exists() {
                return Some(config_at_git_root);
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
    fn lexical_root_matching_does_not_touch_the_filesystem() {
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
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_prefix_spellings_preserve_internal_links_without_filesystem_probes() {
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
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn outside_parent_resolution_is_shared_and_stops_at_first_existing_parent() {
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
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 1);

        // Missing parent components are cached too; each new component is
        // probed once, not once per file or override.
        for name in ["first.dart", "second.dart"] {
            let path = outside.join("missing/deeper").join(name);
            let path = path.to_string_lossy();
            assert!(!config.files.includes.admits(&path));
            for entry in &config.overrides {
                assert!(!entry.matches(&path));
            }
        }
        assert_eq!(root.canonicalizations.load(Ordering::Relaxed), 3);
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
    use super::{FalconConfig, RuleConfiguration, RuleOptions};
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
}
