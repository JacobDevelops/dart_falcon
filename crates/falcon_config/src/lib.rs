//! falcon.json configuration schema and loader (biome 2.x-shaped).
//!
//! `FalconConfig` is the contract; every field and its default is documented
//! here. File rules are grouped by category under `linter.rules`; enablement is
//! resolved from an explicit per-rule level, the `recommended` preset, and
//! per-domain gating (see [`LinterConfig::resolve_rule`]). Project-level
//! (cross-file) rules are a separate feature under `project.rules`, resolved the
//! same way minus domains (see [`ProjectConfig::resolve_rule`]).

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
    /// Project-level (cross-file) rule enablement and levels. A separate feature
    /// from `linter`: these rules reason across the whole file set (unused files,
    /// unused code, call-site nullability) and run only in the CLI project pass.
    pub project: ProjectConfig,
    /// Per-path rule re-configuration (biome `overrides`). Each entry re-patches
    /// the base linter and/or project resolution for files its `includes` match;
    /// later entries win over earlier ones, and all win over the base config.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    /// Maximum number of errors before stopping. Defaults to None (unlimited).
    pub max_errors: Option<usize>,
}

/// One `overrides` entry: a path filter plus a partial linter that re-patches
/// the base resolution for matching files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Override {
    /// Glob patterns selecting the files this override applies to. Same syntax
    /// as `files.includes`: plain entries are positive includes, `!`-prefixed
    /// entries are exclusions. Paths are matched as walked (see
    /// [`FilesConfig::include_patterns`] for the relative-path caveat).
    pub includes: Vec<String>,
    /// Partial linter (file-rule) configuration applied to matching files. Rule
    /// levels, per-rule `options`, and an optional `enabled` master switch are
    /// honored. Options replace the base rule's options for matching files (see
    /// [`FalconConfig::rule_options_for`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linter: Option<OverrideRules>,
    /// Partial project (cross-file) rule configuration applied to matching files.
    /// Same shape and semantics as `linter`, resolved against `project.rules`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<OverrideRules>,
}

/// The partial rule block permitted inside an override: a master switch and
/// per-group rule levels. Shared by the override's `linter` and `project`
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
    /// Whether this override applies to `path` (walked-path form, matching the
    /// diagnostic's `file_path`). A file matches when it is not excluded by any
    /// `!`-pattern and either matches a positive pattern or none are given.
    pub fn matches(&self, path: &str) -> bool {
        let mut positives = Vec::new();
        let mut negatives = Vec::new();
        for pat in &self.includes {
            if let Some(neg) = pat.strip_prefix('!') {
                if let Ok(p) = Pattern::new(neg) {
                    negatives.push(p);
                }
            } else if let Ok(p) = Pattern::new(pat) {
                positives.push(p);
            }
        }
        if negatives.iter().any(|p| p.matches(path)) {
            return false;
        }
        positives.is_empty() || positives.iter().any(|p| p.matches(path))
    }
}

/// `files.includes`: a mixed list of positive include globs and `!`-prefixed
/// exclusions (biome semantics).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    /// Glob patterns. Entries starting with `!` are exclusions; the rest are
    /// positive includes. Empty means "include everything".
    pub includes: Vec<String>,
}

impl FilesConfig {
    /// Exclusion globs (the `!`-prefixed entries, with the `!` stripped).
    pub fn exclude_patterns(&self) -> Vec<String> {
        self.includes
            .iter()
            .filter_map(|p| p.strip_prefix('!').map(str::to_string))
            .collect()
    }

    /// Positive include globs. An empty list — or a catch-all entry (`**` or
    /// `**/*`) — means no positive filtering (include everything), so this
    /// returns an empty vec in that case.
    ///
    /// Positive globs match paths as walked from the CLI argument (e.g. `lib/**`
    /// matches when running `falcon check .` from the project root). Absolute
    /// patterns match absolute paths.
    pub fn include_patterns(&self) -> Vec<String> {
        let positives: Vec<String> = self
            .includes
            .iter()
            .filter(|p| !p.starts_with('!'))
            .cloned()
            .collect();
        if positives.iter().any(|p| p == "**" || p == "**/*") {
            return Vec::new();
        }
        positives
    }
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

/// `project`: master switch and rule levels for project-level (cross-file) rules.
///
/// A separate top-level feature from `linter`. Resolution mirrors the linter's
/// per-rule/recommended logic but has **no domain gating** — project rules are
/// not domain-scoped. When `enabled` is false, every project rule resolves off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Master switch. When false, every project rule resolves to disabled.
    pub enabled: bool,
    /// Rule levels grouped by category, plus the `recommended` preset.
    pub rules: Rules,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Rules::default(),
        }
    }
}

impl ProjectConfig {
    /// Resolve a project rule's effective severity, or `None` if disabled.
    ///
    /// Priority (mirrors [`LinterConfig::resolve_rule`] minus domains):
    /// 1. `enabled == false` → disabled.
    /// 2. Explicit per-rule level under its group wins: `off` → disabled;
    ///    `on` → Warn; `info`/`warn`/`error` → that.
    /// 3. Otherwise enabled iff the recommended preset is active.
    pub fn resolve_rule(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> Option<ResolvedSeverity> {
        if !self.enabled {
            return None;
        }
        if let Some(cfg) = self.rules.groups.get(group).and_then(|g| g.get(name)) {
            return level_to_severity(cfg.level());
        }
        let recommended_on = recommended && self.rules.recommended != Some(false);
        recommended_on.then_some(ResolvedSeverity::Warn)
    }
}

/// Map an explicit rule level to a resolved severity (`off` → disabled).
fn level_to_severity(level: RulePlainConfiguration) -> Option<ResolvedSeverity> {
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
fn level_from_str<E: de::Error>(value: &str) -> Result<RulePlainConfiguration, E> {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Resolve a rule's effective severity for a specific `path`: the base
    /// resolution ([`Self::resolve_rule`]) patched by every override whose
    /// `includes` match, applied in order (later wins). An override's explicit
    /// rule entry replaces the base result — turning the rule off, or on at a
    /// severity. `None` means the rule is disabled for this file.
    pub fn resolve_rule_for(
        &self,
        path: &str,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> Option<ResolvedSeverity> {
        // A globally-disabled linter cannot be resurrected by an override.
        if !self.linter.enabled {
            return None;
        }
        let mut result = self.linter.resolve_rule(group, name, recommended, domains);
        for ov in &self.overrides {
            if !ov.matches(path) {
                continue;
            }
            let Some(linter) = &ov.linter else {
                continue;
            };
            if linter.enabled == Some(false) {
                result = None;
                continue;
            }
            if let Some(cfg) = linter.rules.groups.get(group).and_then(|g| g.get(name)) {
                result = level_to_severity(cfg.level());
            }
        }
        result
    }

    /// Resolve a **project** rule's effective severity for a specific `path`: the
    /// base project resolution ([`ProjectConfig::resolve_rule`]) patched by every
    /// override whose `includes` match, applied in order (later wins). Mirrors
    /// [`Self::resolve_rule_for`] but reads the override's `project` block and has
    /// no domain dimension. `None` means the rule is disabled for this file.
    pub fn resolve_project_rule_for(
        &self,
        path: &str,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> Option<ResolvedSeverity> {
        // A globally-disabled project feature cannot be resurrected by an override.
        if !self.project.enabled {
            return None;
        }
        let mut result = self.project.resolve_rule(group, name, recommended);
        for ov in &self.overrides {
            if !ov.matches(path) {
                continue;
            }
            let Some(project) = &ov.project else {
                continue;
            };
            if project.enabled == Some(false) {
                result = None;
                continue;
            }
            if let Some(cfg) = project.rules.groups.get(group).and_then(|g| g.get(name)) {
                result = level_to_severity(cfg.level());
            }
        }
        result
    }

    /// Whether a **project** rule is enabled for any path — the base project
    /// config or any override turns it on. Drives project-rule registration,
    /// mirroring [`Self::is_rule_enabled_anywhere`].
    pub fn is_project_rule_enabled_anywhere(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
    ) -> bool {
        if !self.project.enabled {
            return false;
        }
        if self
            .project
            .resolve_rule(group, name, recommended)
            .is_some()
        {
            return true;
        }
        self.overrides.iter().any(|ov| {
            ov.project.as_ref().is_some_and(|p| {
                p.enabled != Some(false)
                    && p.rules
                        .groups
                        .get(group)
                        .and_then(|g| g.get(name))
                        .is_some_and(|c| c.level() != RulePlainConfiguration::Off)
            })
        })
    }

    /// Whether a rule is enabled for **any** path — the base config or any
    /// override turns it on. Drives rule registration: a rule must be registered
    /// (and thus run) if it could fire for some file, even when the base config
    /// disables it and only an override re-enables it.
    pub fn is_rule_enabled_anywhere(
        &self,
        group: &str,
        name: &str,
        recommended: bool,
        domains: &[&str],
    ) -> bool {
        if !self.linter.enabled {
            return false;
        }
        if self
            .resolve_rule(group, name, recommended, domains)
            .is_some()
        {
            return true;
        }
        self.overrides.iter().any(|ov| {
            ov.linter.as_ref().is_some_and(|l| {
                l.enabled != Some(false)
                    && l.rules
                        .groups
                        .get(group)
                        .and_then(|g| g.get(name))
                        .is_some_and(|c| c.level() != RulePlainConfiguration::Off)
            })
        })
    }
}

impl LinterConfig {
    /// Resolve a rule's effective severity, or `None` if disabled.
    ///
    /// Priority:
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
        if !self.enabled {
            return None;
        }

        if let Some(cfg) = self.rules.groups.get(group).and_then(|g| g.get(name)) {
            return level_to_severity(cfg.level());
        }

        let recommended_on = recommended && self.rules.recommended != Some(false);

        if !domains.is_empty() {
            let enabled = domains.iter().any(|d| {
                match self
                    .domains
                    .get(*d)
                    .copied()
                    .unwrap_or(DomainValue::Recommended)
                {
                    DomainValue::All => true,
                    DomainValue::Recommended => recommended_on,
                    DomainValue::None => false,
                }
            });
            return enabled.then_some(ResolvedSeverity::Warn);
        }

        recommended_on.then_some(ResolvedSeverity::Warn)
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

/// Load a config from a specific file path.
///
/// # Errors
///
/// Returns `ConfigError` if the file cannot be read, if JSON deserialization
/// fails, or if the file uses the legacy flat schema (top-level `rules`,
/// `exclude_patterns`, or `severity_override`) — which serde would otherwise
/// silently accept as an all-defaults config.
pub fn load_config(path: &Path) -> Result<FalconConfig, ConfigError> {
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

    let config: FalconConfig = serde_json::from_value(value)
        .map_err(|e| ConfigError(format!("failed to parse config JSON: {}", e)))?;

    Ok(config)
}

/// Find a config file starting from `start_dir`, following this priority order:
///
/// 1. `start_dir/falcon.json` — if exists, return it
/// 2. Walk parent dirs up to filesystem root looking for `.git` dir; when found, check `<git_root>/falcon.json`
/// 3. `$HOME/.falcon.json` (via `std::env::var("HOME")`)
/// 4. Return None if nothing found
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    // 1. Check start_dir/falcon.json
    let local_config = start_dir.join("falcon.json");
    if local_config.exists() {
        return Some(local_config);
    }

    // 2. Walk parent dirs looking for .git, then check <git_root>/falcon.json
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

    // 3. Check $HOME/.falcon.json
    if let Ok(home) = std::env::var("HOME") {
        let home_config = PathBuf::from(home).join(".falcon.json");
        if home_config.exists() {
            return Some(home_config);
        }
    }

    None
}

/// Load config from discovered location, or return default if not found.
///
/// If config file is found but loading fails, logs a warning to stderr and returns default.
pub fn load_or_default(start_dir: &Path) -> FalconConfig {
    match find_config(start_dir) {
        Some(path) => match load_config(&path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "warning: failed to load config from {}: {}",
                    path.display(),
                    e
                );
                FalconConfig::default()
            }
        },
        None => FalconConfig::default(),
    }
}
