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
    /// Maximum number of errors before stopping. Defaults to None (unlimited).
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
    /// entries are exclusions. Paths are matched as walked (see
    /// [`FilesConfig::include_patterns`] for the relative-path caveat).
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

/// Immutable override globs, compiled once. Invalid globs are ignored;
/// exclusions win, and no valid positive globs means every path is included.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(from = "Vec<String>")]
pub struct PathFilter {
    patterns: Vec<String>,
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

impl From<Vec<String>> for PathFilter {
    fn from(patterns: Vec<String>) -> Self {
        let negatives = patterns
            .iter()
            .filter(|pattern| pattern.starts_with('!'))
            .count();
        let mut include = Vec::with_capacity(patterns.len() - negatives);
        let mut exclude = Vec::with_capacity(negatives);
        for pattern in &patterns {
            let (target, raw) = match pattern.strip_prefix('!') {
                Some(raw) => (&mut exclude, raw),
                None => (&mut include, pattern.as_str()),
            };
            if let Ok(pattern) = Pattern::new(raw) {
                target.push(pattern);
            }
        }
        Self {
            patterns,
            include,
            exclude,
        }
    }
}

impl Serialize for PathFilter {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.patterns.serialize(serializer)
    }
}

impl PathFilter {
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    pub fn admits(&self, path: &str) -> bool {
        !self.exclude.iter().any(|pattern| pattern.matches(path))
            && (self.include.is_empty() || self.include.iter().any(|pattern| pattern.matches(path)))
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

/// Warnings for a config's top-level keys: a deprecation notice for each legacy
/// spelling, and an "unknown key" notice for anything unrecognized (typo
/// protection). Unknown keys are ignored by serde, so without this a mistyped
/// section vanishes silently. Legacy-flat-schema keys are handled earlier (hard
/// error) and never reach here.
fn top_level_key_warnings(value: &serde_json::Value) -> Vec<String> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    let mut warnings: Vec<String> = obj
        .keys()
        .filter(|k| !KNOWN_TOP_LEVEL_KEYS.contains(&k.as_str()))
        .map(|key| {
            match LEGACY_TOP_LEVEL_KEYS
                .iter()
                .find(|(legacy, _)| legacy == key)
            {
                Some((_, canonical)) => format!(
                    "warning: falcon.json uses the deprecated top-level key `{key}`; it \
                     still works but is a legacy spelling of `{canonical}` — run \
                     `falcon migrate` to update"
                ),
                None => format!(
                    "warning: falcon.json contains unknown top-level key `{key}`; it is ignored"
                ),
            }
        })
        .collect();
    // Override entries accept `project`/`cross_file` as serde aliases for
    // `cross-file`; they load, but deserve the same deprecation nudge.
    if let Some(overrides) = obj.get("overrides").and_then(|v| v.as_array()) {
        for (i, entry) in overrides.iter().enumerate() {
            let Some(entry) = entry.as_object() else {
                continue;
            };
            for &(legacy, _) in LEGACY_TOP_LEVEL_KEYS
                .iter()
                .filter(|(_, canonical)| *canonical == "cross-file")
            {
                if entry.contains_key(legacy) {
                    warnings.push(format!(
                        "warning: falcon.json `overrides[{i}]` uses the deprecated key \
                         `{legacy}`; it still works but is a legacy spelling of \
                         `cross-file` — run `falcon migrate` to update"
                    ));
                }
            }
        }
    }
    warnings
}

/// Load a config from a specific file path.
///
/// Prints a stderr warning for any unknown top-level key (typo protection) and
/// for any legacy key spelling (`project`, `cross_file`, `max_errors`), which
/// still load but are deprecated.
///
/// # Errors
///
/// Returns `ConfigError` if the file cannot be read, if JSON deserialization
/// fails, if the file uses the legacy flat schema (top-level `rules`,
/// `exclude_patterns`, or `severity_override`) — which serde would otherwise
/// silently accept as an all-defaults config — or if `max-errors` is `0`, which
/// would suppress every diagnostic.
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

    for warning in top_level_key_warnings(&value) {
        eprintln!("{warning}");
    }

    let config: FalconConfig = serde_json::from_value(value)
        .map_err(|e| ConfigError(format!("failed to parse config JSON: {}", e)))?;

    if config.max_errors == Some(0) {
        return Err(ConfigError(
            "`max-errors` must be at least 1; 0 would suppress every diagnostic and pass a \
             run with violations present. Omit it (or use null) for no limit."
                .to_string(),
        ));
    }

    Ok(config)
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

/// Load discovered config or defaults; errors in a discovered config propagate.
pub fn load_or_default(start_dir: &Path) -> Result<FalconConfig, ConfigError> {
    match find_config(start_dir) {
        Some(path) => load_config(&path),
        None => Ok(FalconConfig::default()),
    }
}

#[cfg(test)]
mod warning_tests {
    use super::top_level_key_warnings;
    use serde_json::json;

    /// Finding 3: an unknown top-level section (a mistyped `linter`) is warned
    /// about by name, not silently dropped.
    #[test]
    fn unknown_top_level_key_is_warned() {
        let warnings = top_level_key_warnings(&json!({ "linterr": {}, "totallyBogus": 5 }));
        assert_eq!(warnings.len(), 2);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("unknown top-level key `linterr`"))
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("unknown top-level key `totallyBogus`"))
        );
    }

    /// An override entry using a legacy alias (`project`/`cross_file`) for
    /// `cross-file` earns the same deprecation warning, indexed by entry.
    #[test]
    fn override_level_legacy_alias_is_deprecation_warned() {
        let warnings = top_level_key_warnings(&json!({
            "linter": {},
            "overrides": [
                { "includes": ["test/**"], "cross-file": { "enabled": false } },
                { "includes": ["lib/**"], "project": { "enabled": false } }
            ]
        }));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("overrides[1]"));
        assert!(warnings[0].contains("deprecated"));
        assert!(warnings[0].contains("`project`"));
        assert!(warnings[0].contains("falcon migrate"));
    }

    /// Findings 3/4: legacy spellings load but earn a deprecation warning naming
    /// their canonical key and pointing at `falcon migrate`.
    #[test]
    fn legacy_spellings_are_deprecation_warned_not_unknown() {
        let warnings = top_level_key_warnings(&json!({
            "project": {}, "cross_file": {}, "max_errors": 10
        }));
        assert_eq!(warnings.len(), 3);
        for w in &warnings {
            assert!(
                w.contains("deprecated"),
                "should be a deprecation, not unknown: {w}"
            );
            assert!(w.contains("falcon migrate"));
        }
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("`project`") && w.contains("`cross-file`"))
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("`cross_file`") && w.contains("`cross-file`"))
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("`max_errors`") && w.contains("`max-errors`"))
        );
    }

    /// Canonical keys produce no noise.
    #[test]
    fn canonical_keys_produce_no_warnings() {
        let warnings = top_level_key_warnings(&json!({
            "$schema": "x", "files": {}, "linter": {}, "cross-file": {},
            "overrides": [], "max-errors": 50
        }));
        assert!(warnings.is_empty(), "unexpected: {warnings:?}");
    }
}
