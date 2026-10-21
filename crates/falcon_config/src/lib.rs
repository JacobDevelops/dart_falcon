//! falcon.json configuration schema and loader (biome 2.x-shaped).
//!
//! `FalconConfig` is the contract; every field and its default is documented
//! here. File rules are grouped by category under `linter.rules`; enablement is
//! resolved from an explicit per-rule level, the `recommended` preset, and
//! per-domain gating (see [`LinterConfig::resolve_rule`]). Cross-file rules are a
//! separate feature under `cross_file.rules`, resolved the same way minus domains
//! (see [`CrossFileConfig::resolve_rule`]).

use glob::Pattern;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

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
    #[serde(rename = "max-errors", alias = "max_errors")]
    pub max_errors: Option<usize>,
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
    /// [`FalconConfig::rule_options_for`]).
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
    base_dir: Option<PathBuf>,
    lexical_base: Option<PathBuf>,
    working_dir: Option<PathBuf>,
    warnings: Vec<String>,
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
        self.base_dir = Some(std::fs::canonicalize(&lexical).unwrap_or_else(|_| lexical.clone()));
        self.lexical_base = Some(lexical);
        self.working_dir = Some(cwd);
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
            let (target, raw) = match pattern.strip_prefix('!') {
                Some(raw) => (&mut self.exclude, raw),
                None => (&mut self.include, pattern.as_str()),
            };
            let resolved = if Path::new(raw).is_absolute() {
                let path = normalize_path(Path::new(raw));
                let path = if let (Some(lexical), Some(canonical)) =
                    (&self.lexical_base, &self.base_dir)
                    && let Ok(suffix) = path
                        .strip_prefix(lexical)
                        .or_else(|_| path.strip_prefix(canonical))
                {
                    Path::new(&Pattern::escape(&lexical.to_string_lossy())).join(suffix)
                } else {
                    path
                };
                Some(path.to_string_lossy().into_owned())
            } else {
                self.lexical_base.as_ref().map(|base| {
                    let escaped = Pattern::escape(&base.to_string_lossy());
                    let path = normalize_path(&Path::new(&escaped).join(raw));
                    path.to_string_lossy().into_owned()
                })
            };
            match Pattern::new(resolved.as_deref().unwrap_or(raw)) {
                Ok(pattern) => target.push(pattern),
                Err(error) => self
                    .warnings
                    .push(format!("invalid path pattern `{raw}`: {error}")),
            }
        }
        if self.base_dir.is_none()
            && self
                .patterns
                .iter()
                .any(|pattern| pattern == "**" || pattern == "**/*")
        {
            self.include.clear();
        }
    }

    fn matching_path<'a>(&self, path: &'a str) -> std::borrow::Cow<'a, str> {
        let Some(cwd) = &self.working_dir else {
            return path.into();
        };
        let input = Path::new(path);
        let normalized = if !input.is_absolute() {
            std::borrow::Cow::Owned(normalize_path(&cwd.join(input)))
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
        if let (Some(lexical), Some(canonical)) = (&self.lexical_base, &self.base_dir)
            && lexical != canonical
            && !normalized.starts_with(lexical)
            && let Ok(suffix) = normalized.strip_prefix(canonical)
        {
            return lexical.join(suffix).to_string_lossy().into_owned().into();
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
        let raw = path.to_string_lossy();
        let mut path = self.matching_path(&raw).into_owned();
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
        self.files.includes.base_dir.as_deref()
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
        for entry in &mut self.overrides {
            entry.includes.set_base_dir(base);
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
    /// Per-domain gating (e.g. `{"flutter": "recommended"}`).
    pub domains: BTreeMap<String, DomainValue>,
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
        options: serde_json::Value,
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

    /// The configured options, or `None` if absent. The options-less
    /// `{ "level": "warn" }` form normalizes to `None`, matching the plain form.
    pub fn options(&self) -> Option<&serde_json::Value> {
        match self {
            RuleConfiguration::WithOptions { options, .. } if !options.is_null() => Some(options),
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
                let mut options: Option<serde_json::Value> = None;
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
                    options: options.unwrap_or(serde_json::Value::Null),
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

    /// Return `rule_name`'s base (non-path-scoped) options if it is configured
    /// under `group` with the `WithOptions` form. Scoped to the rule's own group
    /// so lookup stays consistent with [`LinterConfig::resolve_rule`]: an entry
    /// placed under the wrong group is ignored here just as its level is ignored
    /// there. Ignores overrides — see [`Self::rule_options_for`] for the
    /// path-aware resolution rules actually use.
    pub fn rule_options(&self, group: &str, rule_name: &str) -> Option<&serde_json::Value> {
        self.linter
            .rules
            .groups
            .get(group)
            .and_then(|g| g.get(rule_name))
            .and_then(RuleConfiguration::options)
    }

    /// Return `rule_name`'s effective options for a specific `path`: the base
    /// options ([`Self::rule_options`]) replaced by every override whose
    /// `includes` match `path` and that specifies options for the rule, applied
    /// in order (later wins).
    ///
    /// Semantics mirror per-path level resolution ([`Self::resolve_rule_for`]):
    /// a matching override's `options` block **replaces** the base options
    /// wholesale — options are not deep-merged. An override that sets only a
    /// level (no `options`) leaves the base options intact.
    pub fn rule_options_for(
        &self,
        path: &str,
        group: &str,
        rule_name: &str,
    ) -> Option<&serde_json::Value> {
        let mut result = self.rule_options(group, rule_name);
        for ov in &self.overrides {
            if !ov.matches(path) {
                continue;
            }
            if let Some(opts) = ov
                .linter
                .as_ref()
                .and_then(|l| l.rules.groups.get(group))
                .and_then(|g| g.get(rule_name))
                .and_then(RuleConfiguration::options)
            {
                result = Some(opts);
            }
        }
        result
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

/// Error type for configuration loading and parsing.
#[derive(Debug)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "config error: {}", self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Canonical top-level keys of a falcon.json.
pub const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
    "$schema",
    "files",
    "linter",
    "cross-file",
    "overrides",
    "max-errors",
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
    path: &str,
    kind: ConfigObject,
    warnings: &mut Vec<String>,
) -> Result<(), ConfigError> {
    let Some(object) = value.as_object() else {
        return Ok(()); // Serde reports wrong value types.
    };
    for (key, value) in object {
        let full_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
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
                "falcon.json uses deprecated key `{full_path}` (`{key}`); \
                 use `{canonical}` — run `falcon migrate` to update"
            ));
        }
        use ConfigObject as Object;
        let child = match (kind, canonical) {
            (Object::Root, "$schema" | "max-errors")
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
                        validate_keys(
                            entry,
                            &format!("{full_path}[{index}]"),
                            Object::Override,
                            warnings,
                        )?;
                    }
                }
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
                return Err(ConfigError(format!("{description} `{full_path}`")));
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
        .map_err(|e| ConfigError(format!("failed to read config file: {}", e)))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| ConfigError(format!("failed to parse config JSON: {}", e)))?;

    if let Some(obj) = value.as_object()
        && ["rules", "exclude_patterns", "severity_override"]
            .iter()
            .any(|k| obj.contains_key(*k))
    {
        return Err(ConfigError(
            "falcon.json uses the legacy flat schema; migrate to the biome-style schema \
             (\"linter.rules\" grouped by category, \"files.includes\"). See docs/configuration.md"
                .to_string(),
        ));
    }

    let mut warnings = Vec::new();
    validate_keys(&value, "", ConfigObject::Root, &mut warnings)?;

    let mut config: FalconConfig = serde_json::from_value(value)
        .map_err(|e| ConfigError(format!("failed to parse config JSON: {}", e)))?;

    if config.max_errors == Some(0) {
        return Err(ConfigError(
            "`max-errors` must be at least 1; use the CLI --max-errors 0 flag to hide output. \
             Omit it (or use null) for no limit."
                .to_string(),
        ));
    }

    config.resolve_paths(path.parent().unwrap_or(Path::new(".")));
    warnings.extend(config.path_warnings());
    Ok(LoadedConfig { config, warnings })
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
            let error = validate_keys(&value, "", ConfigObject::Root, &mut Vec::new())
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
        validate_keys(&value, "", ConfigObject::Root, &mut warnings).unwrap();
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
}
