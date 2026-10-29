//! `falcon migrate`: convert a dart_code_linter / pyramid_lint
//! `analysis_options.yaml` into an equivalent biome-style `falcon.json`,
//! mirroring `biome migrate eslint/prettier`.
//!
//! The mapping table is the rule metadata itself: [`RuleSource`] carries each
//! rule's upstream id, so we invert it (upstream id → falcon `name`/`group`/
//! `cross_file`) and route every configured upstream rule to its falcon slot.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use falcon_config::{
    ConfigPathSegment, KNOWN_TOP_LEVEL_KEYS, LEGACY_TOP_LEVEL_KEYS, RulePlainConfiguration,
    deserialize_config_value, format_config_path,
};
use falcon_rules::meta::{
    RULE_METADATA, RuleMeta, RuleSource, canonical_rule_name, canonicalize_rule_entries,
    canonicalize_rule_map, meta_for,
};
use falcon_rules::schema::SCHEMA_URL;
use falcon_rules::{canonicalize_loaded_config, canonicalize_option_keys};
use serde_json::{Map, Value, json};

/// Upstream pyramid_lint ids whose rules were merged into a surviving
/// dart_code_linter rule during twin unification. Their metadata now carries the
/// dart_code_linter `source`, so [`build_lookups`] would otherwise drop these
/// upstream ids; they are re-added to the pyramid lookup so `falcon migrate`
/// still maps a `custom_lint` config that used them.
const MERGED_PYRAMID_UPSTREAM: &[(&str, &str)] = &[
    ("no_empty_block", "no-empty-block"),
    ("avoid_empty_blocks", "no-empty-block"),
    ("no_magic_number", "no-magic-number"),
    ("avoid_unused_parameters", "avoid-unused-parameters"),
];

/// Outcome of a migration: the generated falcon.json plus a report of what was
/// and was not mapped.
pub struct MigrationResult {
    /// Pretty-printed falcon.json with a trailing newline.
    pub json: String,
    /// Rule ids that matched no falcon rule and were discarded.
    pub unrecognized: Vec<String>,
    /// Number of mapped upstream rules or rewritten legacy rule ids in an upgrade.
    pub migrated_count: usize,
    /// Loader and canonicalization warnings requiring manual review.
    pub warnings: Vec<String>,
    /// Deprecated option-key warnings, one per rewritten key.
    pub option_key_rewrites: Vec<String>,
}

/// The falcon config for one upstream entry, before JSON serialization.
enum RuleConfig {
    Disabled,
    Enabled(Option<Value>),
}

impl From<&Value> for RuleConfig {
    fn from(value: &Value) -> Self {
        match value {
            Value::Bool(false) => Self::Disabled,
            Value::Bool(true) | Value::Null => Self::Enabled(None),
            _ => Self::Enabled(Some(value.clone())),
        }
    }
}

/// An upstream-id → falcon-metadata lookup for one source linter.
type RuleLookup = BTreeMap<&'static str, &'static RuleMeta>;

/// Invert [`RULE_METADATA`] into (dcl-id → meta, pyramid-id → meta, lints-id →
/// meta) lookups.
fn build_lookups() -> (RuleLookup, RuleLookup, RuleLookup) {
    let mut dcl = BTreeMap::new();
    let mut pyramid = BTreeMap::new();
    let mut lints = BTreeMap::new();
    for meta in RULE_METADATA {
        match meta.source {
            RuleSource::DartCodeLinter(id) => {
                dcl.insert(id, meta);
            }
            RuleSource::PyramidLint(id) => {
                pyramid.insert(id, meta);
            }
            RuleSource::Lints(id) => {
                lints.insert(id, meta);
            }
            RuleSource::Falcon => {}
        }
    }
    // Re-map the pyramid ids of the merged twins onto their surviving rule.
    for &(upstream, canonical) in MERGED_PYRAMID_UPSTREAM {
        if let Some(meta) = meta_for(canonical) {
            pyramid.insert(upstream, meta);
        }
    }
    (dcl, pyramid, lints)
}

#[derive(Default)]
struct Migration {
    linter_groups: BTreeMap<String, Vec<(String, Value)>>,
    cross_file_groups: BTreeMap<String, Vec<(String, Value)>>,
    overrides: Vec<Value>,
    unrecognized: Vec<String>,
    migrated_count: usize,
    warnings: Vec<String>,
    option_key_rewrites: Vec<String>,
}

impl Migration {
    fn place_rules(&mut self, rules: &Value, source: &str, lookup: &RuleLookup) {
        match rules {
            Value::Array(entries) => {
                for (index, entry) in entries.iter().enumerate() {
                    match entry {
                        Value::String(id) => self.place(
                            id.clone(), RuleConfig::Enabled(None), entry, lookup, source, Some(index),
                        ),
                        Value::Object(map) if !map.is_empty() => {
                            for (id, value) in map {
                                self.place(
                                    id.clone(), RuleConfig::from(value), value, lookup, source, Some(index),
                                );
                            }
                        }
                        _ => self.warnings.push(format!(
                            "unsupported upstream rule entry `{source}.rules[{index}]`; discarded value {entry}"
                        )),
                    }
                }
            }
            Value::Object(map) => {
                for (id, value) in map {
                    self.place(
                        id.clone(),
                        RuleConfig::from(value),
                        value,
                        lookup,
                        source,
                        None,
                    );
                }
            }
            _ => self.warnings.push(format!(
                "unsupported upstream rules `{source}.rules`; discarded value {rules}"
            )),
        }
    }

    fn place(
        &mut self,
        id: String,
        cfg: RuleConfig,
        value: &Value,
        lookup: &RuleLookup,
        source: &str,
        index: Option<usize>,
    ) {
        let path = match index {
            Some(index) => format!("{source}.rules[{index}].{id}"),
            None => format!("{source}.rules.{id}"),
        };
        let Some(meta) = lookup.get(id.as_str()) else {
            self.warnings.push(format!(
                "unknown upstream rule `{path}`; discarded value {value}"
            ));
            self.unrecognized.push(id);
            return;
        };
        let cfg = match cfg {
            RuleConfig::Enabled(Some(mut options)) => {
                if let Some(excludes) = options
                    .as_object_mut()
                    .and_then(|map| map.shift_remove("exclude"))
                    && !excludes.as_array().is_some_and(Vec::is_empty)
                {
                    let has_positive = |pattern: &Value| {
                        pattern.as_str().is_some_and(|text| !text.starts_with('!'))
                    };
                    if !has_positive(&excludes)
                        && !excludes
                            .as_array()
                            .is_some_and(|patterns| patterns.iter().any(has_positive))
                    {
                        self.warnings.push(format!(
                            "cannot migrate `{path}.exclude` without a positive pattern; discarded value {excludes}; keeping the rule enabled"
                        ));
                    } else {
                        let section = if meta.is_cross_file() {
                            "cross-file"
                        } else {
                            "linter"
                        };
                        self.overrides.push(json!({
                            "includes": excludes,
                            section: { "rules": { meta.group.as_str(): { meta.name: "off" } } }
                        }));
                    }
                }
                if options.as_object().is_some_and(Map::is_empty) {
                    RuleConfig::Enabled(None)
                } else {
                    RuleConfig::Enabled(Some(options))
                }
            }
            cfg => cfg,
        };
        let target = if meta.is_cross_file() {
            &mut self.cross_file_groups
        } else {
            &mut self.linter_groups
        };
        // Preserve aliases until the shared merge so canonical names and alias
        // precedence break equal-severity ties exactly as configuration loading.
        let name = if canonical_rule_name(&id) == meta.name {
            id
        } else {
            meta.name.to_string()
        };
        target
            .entry(meta.group.as_str().to_string())
            .or_default()
            .push((name, config_to_json(cfg)));
        self.migrated_count += 1;
    }
}

/// Serialize a [`RuleConfig`] to its falcon.json value: `"off"`, `"warn"`, or
/// `{ "level": "warn", "options": {...} }`.
fn config_to_json(cfg: RuleConfig) -> Value {
    match cfg {
        RuleConfig::Disabled => json!(RulePlainConfiguration::Off),
        RuleConfig::Enabled(None) => json!(RulePlainConfiguration::Warn),
        RuleConfig::Enabled(Some(options)) => {
            json!({ "level": RulePlainConfiguration::Warn, "options": options })
        }
    }
}

/// Assemble a section's `rules` object: `recommended: false` plus each non-empty
/// group. Groups are omitted when they contain no migrated rules.
fn build_rules_object(
    groups: BTreeMap<String, Vec<(String, Value)>>,
    path: &str,
    report: &mut Migration,
) -> Value {
    let mut rules = Map::new();
    rules.insert("recommended".into(), json!(false));
    for (group, entries) in groups {
        if !entries.is_empty() {
            let group_path = format!("{path}.rules.{group}");
            let (mut entries, warnings) = canonicalize_rule_entries(entries, &group_path);
            report.warnings.extend(warnings);
            canonicalize_group_options(&mut entries, &group_path, report);
            rules.insert(group, Value::Object(entries));
        }
    }
    json!({ "rules": Value::Object(rules) })
}

/// Convert an `analysis_options.yaml` body into an equivalent falcon.json.
///
/// # Errors
///
/// Returns an error string if the YAML cannot be parsed or the result cannot be
/// serialized. Unsupported or invalid options are discarded with their full
/// paths and values reported, so the written config passes canonical loading.
pub fn migrate_yaml_to_config(yaml: &str) -> Result<MigrationResult, String> {
    let doc: Value =
        serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse YAML: {e}"))?;
    let (dcl, pyramid, lints) = build_lookups();

    let mut migration = Migration::default();

    // dart_code_linter rules live under `dart_code_linter.rules`; pyramid_lint is
    // a custom_lint plugin, so its rules live under `custom_lint.rules`. Both use
    // the YAML list form.
    let sections = [("dart_code_linter", &dcl), ("custom_lint", &pyramid)];
    for (section, lookup) in sections {
        let Some(rules) = doc.get(section).and_then(|s| s.get("rules")) else {
            continue;
        };
        migration.place_rules(rules, section, lookup);
    }

    // Official `package:lints` / `package:flutter_lints` rules live under the
    // top-level `linter.rules` key (list or map form).
    if let Some(rules) = doc.get("linter").and_then(|s| s.get("rules")) {
        migration.place_rules(rules, "linter", &lints);
    }

    let mut root = Map::new();
    root.insert("$schema".into(), json!(SCHEMA_URL));
    root.insert(
        "linter".into(),
        build_rules_object(
            std::mem::take(&mut migration.linter_groups),
            "linter",
            &mut migration,
        ),
    );
    // Only emit the cross-file block when a cross-file rule was actually migrated.
    if !migration.cross_file_groups.is_empty() {
        root.insert(
            "cross-file".into(),
            build_rules_object(
                std::mem::take(&mut migration.cross_file_groups),
                "cross-file",
                &mut migration,
            ),
        );
    }
    if !migration.overrides.is_empty() {
        root.insert(
            "overrides".into(),
            Value::Array(std::mem::take(&mut migration.overrides)),
        );
    }

    let json = serialize_config(Value::Object(root), &mut migration.warnings, false)?;

    Ok(MigrationResult {
        json,
        unrecognized: migration.unrecognized,
        migrated_count: migration.migrated_count,
        warnings: migration.warnings,
        option_key_rewrites: migration.option_key_rewrites,
    })
}

/// Whether `content` is an existing falcon.json (rather than an upstream
/// `analysis_options.yaml`): it parses as a JSON object carrying at least one
/// recognizable falcon.json top-level key.
fn looks_like_falcon_json(content: &str) -> bool {
    serde_json::from_str::<Value>(content)
        .ok()
        .as_ref()
        .and_then(Value::as_object)
        .is_some_and(|obj| {
            KNOWN_TOP_LEVEL_KEYS
                .iter()
                .copied()
                .chain(LEGACY_TOP_LEVEL_KEYS.iter().map(|(legacy, _)| *legacy))
                .chain(["rules", "exclude_patterns", "severity_override"])
                .filter(|key| !matches!(*key, "max-errors" | "max_errors"))
                .any(|key| obj.contains_key(key))
        })
}

/// Upgrade an existing falcon.json in place: canonicalize legacy top-level key
/// spellings to kebab-case: the cross-file section (`project`/`cross_file` →
/// `cross-file`, including each override's block) and `max_errors` → `max-errors`,
/// then rewrite legacy rule ids (the pre-1.0 `snake_case` ids and the removed
/// twin variants) to their canonical ids across `linter`/`cross-file` rule maps
/// and every override, preserving each rule's level and canonicalizing nested
/// declared option keys to kebab-case. When a legacy id and its canonical id (or
/// two legacy twins) collide in the same group, the more severe level wins.
///
/// # Errors
///
/// Returns an error string if the input is not valid JSON or the result cannot be
/// serialized. Unsupported or invalid entries are discarded with their full
/// paths and values reported, so the written config passes canonical loading.
pub fn migrate_existing_config(json: &str) -> Result<MigrationResult, String> {
    let mut root: Value =
        serde_json::from_str(json).map_err(|e| format!("failed to parse falcon.json: {e}"))?;
    let mut report = Migration::default();
    let legacy_flat = ["rules", "severity_override", "exclude_patterns"]
        .iter()
        .any(|key| root.get(key).is_some());
    for &(legacy, canonical) in LEGACY_TOP_LEVEL_KEYS {
        rename_key(&mut root, legacy, canonical, "", &mut report.warnings);
    }
    migrate_flat_config(&mut root, &mut report)?;
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for (index, ov) in overrides.iter_mut().enumerate() {
            rename_cross_file_section(ov, &format!("overrides[{index}]"), &mut report.warnings);
        }
    }

    relocate_rules(&mut root, "", &mut report)?;
    for section in ["linter", "cross-file"] {
        if let Some(s) = root.get_mut(section) {
            canonicalize_section_rules(s, section, &mut report);
        }
    }
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for (index, ov) in overrides.iter_mut().enumerate() {
            relocate_rules(ov, &format!("overrides[{index}]"), &mut report)?;
            for section in ["linter", "cross-file"] {
                if let Some(s) = ov.get_mut(section) {
                    canonicalize_section_rules(
                        s,
                        &format!("overrides[{index}].{section}"),
                        &mut report,
                    );
                }
            }
        }
    }

    let out = serialize_config(root, &mut report.warnings, legacy_flat)?;
    Ok(MigrationResult {
        json: out,
        unrecognized: report.unrecognized,
        migrated_count: report.migrated_count,
        warnings: report.warnings,
        option_key_rewrites: report.option_key_rewrites,
    })
}

fn migrate_flat_config(root: &mut Value, report: &mut Migration) -> Result<(), String> {
    for key in ["rules", "severity_override"] {
        let Some(entries) = root.get_mut(key).map(std::mem::take) else {
            continue;
        };
        let Value::Object(mut entries) = entries else {
            return Err(format!(
                "cannot migrate legacy `{key}`: expected a rule object"
            ));
        };
        if key == "severity_override" {
            for (name, setting) in &mut entries {
                let level = setting.as_str().ok_or_else(|| {
                    format!("cannot migrate legacy `{key}.{name}`: expected a severity string")
                })?;
                let normalized = match level.to_ascii_lowercase().as_str() {
                    "warning" => "warn".to_owned(),
                    "note" => "info".to_owned(),
                    level => level.to_owned(),
                };
                falcon_config::level_from_str::<serde_json::Error>(&normalized)
                    .map_err(|error| format!("cannot migrate legacy `{key}.{name}`: {error}"))?;
                *setting = Value::String(normalized);
            }
            let (canonical, warnings) = canonicalize_rule_entries(entries, key);
            entries = canonical;
            report.warnings.extend(warnings);
        }
        for (name, setting) in &entries {
            let meta = meta_for(name)
                .ok_or_else(|| format!("cannot migrate legacy `{key}.{name}`: no rule mapping"))?;
            let config = legacy_rule_setting(key, name, setting)?;
            let section = if meta.is_cross_file() {
                "cross-file"
            } else {
                "linter"
            };
            let mut target = &mut *root;
            for part in [section, "rules", meta.group.as_str()] {
                target = target
                    .as_object_mut()
                    .ok_or_else(|| {
                        format!("cannot migrate legacy `{key}` into a non-object section")
                    })?
                    .entry(part)
                    .or_insert_with(|| json!({}));
            }
            let entries = target
                .as_object_mut()
                .ok_or_else(|| format!("cannot migrate legacy `{key}` into a non-object group"))?;
            let group_path = format!("{section}.rules.{}", meta.group.as_str());
            merge_legacy_rule(key, name, meta.name, config, entries, &group_path, report);
            report.migrated_count += 1;
        }
        root.as_object_mut()
            .expect("legacy key in object")
            .shift_remove(key);
    }
    migrate_flat_excludes(root)?;
    Ok(())
}

fn legacy_rule_setting(key: &str, name: &str, setting: &Value) -> Result<Value, String> {
    let config = if key == "severity_override" {
        setting.clone()
    } else {
        let object = setting
            .as_object()
            .ok_or_else(|| format!("cannot migrate legacy `{key}.{name}`: expected an object"))?;
        if let Some(unknown) = object
            .keys()
            .find(|key| !matches!(key.as_str(), "enabled" | "options"))
        {
            return Err(format!(
                "cannot migrate legacy `{key}.{name}.{unknown}`: no mapping"
            ));
        }
        let enabled = match object.get("enabled") {
            None | Some(Value::Bool(true)) => true,
            Some(Value::Bool(false)) => false,
            _ => {
                return Err(format!(
                    "cannot migrate legacy `{key}.{name}.enabled`: expected a boolean"
                ));
            }
        };
        let level = if enabled { "warn" } else { "off" };
        if let Some(options) = object.get("options") {
            json!({"level":level,"options":options})
        } else {
            json!(level)
        }
    };
    Ok(config)
}

fn merge_legacy_rule(
    key: &str,
    name: &str,
    rule_name: &str,
    config: Value,
    entries: &mut Map<String, Value>,
    group_path: &str,
    report: &mut Migration,
) {
    if key == "severity_override" {
        let (canonical, warnings) = canonicalize_rule_entries(std::mem::take(entries), group_path);
        *entries = canonical;
        report.warnings.extend(warnings);
        let existing = entries.get_mut(rule_name);
        let disabled = existing.as_ref().is_some_and(|existing| {
            existing.as_str() == Some("off")
                || existing.get("level").and_then(Value::as_str) == Some("off")
        });
        if disabled {
            report.warnings.push(format!(
                "ignored legacy `{key}.{name}`: discarded value {config}; keeping the rule disabled"
            ));
        } else {
            match existing {
                Some(Value::Object(existing)) => {
                    existing.insert("level".into(), config);
                }
                Some(existing) => *existing = config,
                None => {
                    entries.insert(rule_name.into(), config);
                }
            }
        }
    } else {
        let (merged, warnings) = canonicalize_rule_entries(
            std::mem::take(entries)
                .into_iter()
                .chain(std::iter::once((name.to_owned(), config))),
            group_path,
        );
        *entries = merged;
        report.warnings.extend(warnings);
    }
}

fn migrate_flat_excludes(root: &mut Value) -> Result<(), String> {
    if let Some(excludes) = root.get_mut("exclude_patterns").map(std::mem::take) {
        let excludes = excludes
            .as_array()
            .ok_or("cannot migrate legacy `exclude_patterns`: expected an array")?;
        let patterns = excludes
            .iter()
            .map(|value| {
                let pattern = value.as_str().ok_or_else(|| {
                    format!("cannot migrate legacy `exclude_patterns`: invalid pattern {value}")
                })?;
                if pattern.starts_with('!')
                    || !falcon_config::PathFilter::from(vec![pattern.to_owned()])
                        .warnings()
                        .is_empty()
                {
                    return Err(format!(
                        "cannot migrate legacy `exclude_patterns`: invalid pattern {value}"
                    ));
                }
                // Legacy single wildcards could cross separators. A leading `*`
                // can be expressed recursively, as can `**/*` at the final segment.
                if pattern.contains('?')
                    || pattern.contains('[')
                    || pattern.match_indices('*').any(|(index, _)| {
                        index != 0
                            && pattern.as_bytes().get(index - 1) != Some(&b'*')
                            && pattern.as_bytes().get(index + 1) != Some(&b'*')
                            && !(pattern[..index].ends_with("**/")
                                && !pattern[index..].contains('/'))
                    })
                {
                    return Err(format!(
                        "cannot migrate legacy `exclude_patterns`: cannot preserve wildcard semantics of {value}"
                    ));
                }
                let prefix = if pattern.starts_with('*') && !pattern.starts_with("**") {
                    "!**/"
                } else {
                    "!"
                };
                Ok(Value::String(format!("{prefix}{pattern}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let files = root
            .as_object_mut()
            .expect("legacy key in object")
            .entry("files")
            .or_insert_with(|| json!({}));
        let files = files
            .as_object_mut()
            .ok_or("cannot migrate legacy `exclude_patterns` into non-object `files`")?;
        let includes = files.entry("includes").or_insert_with(|| json!(["**"]));
        if includes.is_string() {
            *includes = json!([std::mem::take(includes)]);
        }
        includes
            .as_array_mut()
            .ok_or("cannot migrate legacy `exclude_patterns` into invalid `files.includes`")?
            .extend(patterns);
        root.as_object_mut()
            .expect("legacy key in object")
            .shift_remove("exclude_patterns");
    }
    Ok(())
}

fn is_section_path(path: &[ConfigPathSegment]) -> bool {
    use ConfigPathSegment::{Index, Key};
    let in_override = matches!(path, [Key(key), Index(_), ..] if key == "overrides");
    let path = match path {
        [Key(key), Index(_), rest @ ..] if key == "overrides" => rest,
        path => path,
    };
    match path {
        [] => true,
        [Key(key)] => matches!(
            key.as_str(),
            "files" | "linter" | "cross-file" | "overrides"
        ),
        [Key(section), Key(key)] if section == "linter" || section == "cross-file" => {
            key == "rules" || (!in_override && section == "linter" && key == "domains")
        }
        [Key(section), Key(rules), Key(group)]
            if section == "linter" || section == "cross-file" =>
        {
            rules == "rules" && falcon_config::RULE_GROUPS.contains(&group.as_str())
        }
        _ => false,
    }
}

/// Repair invalid entries using the canonical loader before serializing.
fn serialize_config(
    mut value: Value,
    warnings: &mut Vec<String>,
    legacy_flat: bool,
) -> Result<String, String> {
    let mut removals = Vec::new();
    repair_includes(&mut value, warnings, &mut removals)?;
    loop {
        let loaded = deserialize_config_value(&value, Path::new("falcon.json"))
            .and_then(canonicalize_loaded_config);
        match loaded {
            Ok(loaded) => {
                if legacy_flat {
                    warn_legacy_default_rules(&loaded.config, warnings);
                }
                for warning in loaded.warnings {
                    if !warnings.contains(&warning) {
                        warnings.push(warning);
                    }
                }
                break;
            }
            Err(error) => {
                let path = &error.1;
                let original = original_path(path, &removals);
                let original_display = format_config_path(&original);
                let diagnostic = error
                    .0
                    .replace(&format_config_path(path), &original_display);
                if matches!(path.first(), Some(ConfigPathSegment::Key(key)) if key == "files") {
                    return Err(format!(
                        "cannot repair `{original_display}`: {error}; refusing to widen the analyzed file set"
                    ));
                }
                let override_scope = match path.as_slice() {
                    [
                        ConfigPathSegment::Key(key),
                        ConfigPathSegment::Index(_),
                        rest @ ..,
                    ] if key == "overrides" => {
                        rest.is_empty()
                            || !matches!(rest.first(), Some(ConfigPathSegment::Key(key)) if key == "linter" || key == "cross-file")
                    }
                    _ => false,
                };
                let target = if override_scope {
                    &path[..2]
                } else {
                    path.as_slice()
                };
                if !override_scope && is_section_path(target) {
                    return Err(format!(
                        "cannot safely repair `{original_display}`: {error}"
                    ));
                }
                let discarded = discard_at(&mut value, target).ok_or_else(|| {
                    format!("cannot resolve configuration path `{original_display}`: {error}")
                })?;
                let discarded_display = format_config_path(&original_path(target, &removals));
                warnings.push(if override_scope {
                    format!("discarded override `{discarded_display}`: {diagnostic}; discarded value {discarded}; refusing to widen its scope")
                } else {
                    format!("invalid configuration at `{original_display}`: {diagnostic}; discarded value {discarded} at `{discarded_display}`")
                });
                if matches!(target.last(), Some(ConfigPathSegment::Index(_))) {
                    removals.push(target.to_vec());
                }
                remove_empty_rule_options(&mut value, target);
            }
        }
    }
    let mut json = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("failed to serialize falcon.json: {e}"))?;
    json.push('\n');
    Ok(json)
}

fn warn_legacy_default_rules(config: &falcon_config::FalconConfig, warnings: &mut Vec<String>) {
    let affected = RULE_METADATA
        .iter()
        .filter(|meta| {
            let group = meta.group.as_str();
            let (enabled, rules, resolved) = if meta.is_cross_file() {
                (
                    config.cross_file.enabled,
                    &config.cross_file.rules,
                    config
                        .cross_file
                        .resolve_rule(group, meta.name, meta.recommended),
                )
            } else {
                (
                    config.linter.enabled,
                    &config.linter.rules,
                    config.linter.resolve_rule(
                        group,
                        meta.name,
                        meta.recommended,
                        meta.domain_names(),
                    ),
                )
            };
            enabled
                && !rules
                    .groups
                    .get(group)
                    .is_some_and(|rules| rules.contains_key(meta.name))
                && resolved.is_none()
        })
        .count();
    warnings.push(format!(
        "legacy flat configs ran all rules by default; migrated configs run recommended plus explicitly listed rules (subject to section/domain settings). {affected} unlisted rules no longer run by default; enable desired rules explicitly by group in `linter.rules` or `cross-file.rules` using `\"warn\"`, `\"info\"`, or `\"error\"`"
    ));
}

fn remove_empty_rule_options(value: &mut Value, target: &[ConfigPathSegment]) {
    let rule_path = match target {
        [
            ConfigPathSegment::Key(key),
            ConfigPathSegment::Index(_),
            rest @ ..,
        ] if key == "overrides" => rest,
        path => path,
    };
    if let [
        ConfigPathSegment::Key(section),
        ConfigPathSegment::Key(rules),
        ConfigPathSegment::Key(group),
        ConfigPathSegment::Key(rule),
        ConfigPathSegment::Key(options),
        ConfigPathSegment::Key(_),
    ] = rule_path
        && matches!(section.as_str(), "linter" | "cross-file")
        && rules == "rules"
        && falcon_config::RULE_GROUPS.contains(&group.as_str())
        && options == "options"
        && meta_for(rule).is_some_and(|meta| !meta.has_options())
        && let Some(rule) = target[..target.len() - 2]
            .iter()
            .try_fold(value, |value, segment| match segment {
                ConfigPathSegment::Key(key) => value.get_mut(key),
                ConfigPathSegment::Index(index) => value.get_mut(*index),
            })
            .and_then(Value::as_object_mut)
        && rule
            .get("options")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty)
    {
        rule.shift_remove("options");
    }
}

fn repair_includes(
    value: &mut Value,
    warnings: &mut Vec<String>,
    removals: &mut Vec<Vec<ConfigPathSegment>>,
) -> Result<(), String> {
    fn repair(includes: &mut Value, path: &str, warnings: &mut Vec<String>) -> bool {
        if includes.is_string() {
            *includes = Value::Array(vec![std::mem::take(includes)]);
            warnings.push(format!("converted `{path}` string to a one-element array"));
        }
        let Some(patterns) = includes.as_array_mut() else {
            return false;
        };
        let had_positive = patterns
            .iter()
            .any(|pattern| !pattern.as_str().is_some_and(|text| text.starts_with('!')));
        let mut index = 0;
        patterns.retain(|pattern| {
            let valid = pattern.as_str().is_some_and(|pattern| {
                falcon_config::PathFilter::from(vec![pattern.to_owned()])
                    .warnings()
                    .is_empty()
            });
            if !valid {
                warnings.push(format!(
                    "invalid include at `{path}[{index}]`; discarded value {pattern}"
                ));
            }
            index += 1;
            valid
        });
        !had_positive
            || patterns
                .iter()
                .any(|pattern| pattern.as_str().is_some_and(|text| !text.starts_with('!')))
    }

    if let Some(includes) = value
        .get_mut("files")
        .and_then(|files| files.get_mut("includes"))
        && !repair(includes, "files.includes", warnings)
    {
        return Err("cannot repair `files.includes`: no valid positive include remains; refusing to widen the analyzed file set".into());
    }
    if let Some(overrides) = value.get_mut("overrides").and_then(Value::as_array_mut) {
        let mut index = 0;
        while index < overrides.len() {
            let path = vec![
                ConfigPathSegment::Key("overrides".into()),
                ConfigPathSegment::Index(index),
            ];
            let display = format_config_path(&original_path(&path, removals));
            let valid = overrides[index]
                .get_mut("includes")
                .is_none_or(|includes| repair(includes, &format!("{display}.includes"), warnings));
            if valid {
                index += 1;
            } else {
                warnings.push(format!("discarded override `{display}`: includes has no usable positive patterns; refusing to widen its scope"));
                overrides.remove(index);
                removals.push(path);
            }
        }
    }
    Ok(())
}

fn relocate_rules(value: &mut Value, prefix: &str, report: &mut Migration) -> Result<(), String> {
    let mut moves = BTreeMap::<_, Vec<_>>::new();
    for section in ["linter", "cross-file"] {
        let Some(groups) = value
            .get_mut(section)
            .and_then(|value| value.get_mut("rules"))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        for (group, entries) in groups {
            let Some(entries) = entries.as_object_mut() else {
                continue;
            };
            entries.retain(|name, config| {
                let Some(meta) = meta_for(name) else {
                    return true;
                };
                if canonical_rule_name(name) != name {
                    report.migrated_count += 1;
                }
                let target = if meta.is_cross_file() {
                    "cross-file"
                } else {
                    "linter"
                };
                if section == target && group == meta.group.as_str() {
                    return true;
                }
                let old = format!("{section}.rules.{group}.{name}");
                let new = format!("{target}.rules.{}.{}", meta.group.as_str(), meta.name);
                let prefix = if prefix.is_empty() {
                    String::new()
                } else {
                    format!("{prefix}.")
                };
                report
                    .warnings
                    .push(format!("moved rule `{prefix}{old}` to `{prefix}{new}`"));
                moves
                    .entry((target, meta.group.as_str()))
                    .or_default()
                    .push((name.clone(), std::mem::take(config)));
                false
            });
        }
    }
    for ((section, group), moved) in moves {
        let path = if prefix.is_empty() {
            format!("{section}.rules.{group}")
        } else {
            format!("{prefix}.{section}.rules.{group}")
        };
        let mut target = &mut *value;
        let mut target_path = prefix.to_owned();
        for key in [section, "rules", group] {
            if key == section
                && prefix.starts_with("overrides[")
                && target.get(key).is_some_and(Value::is_null)
            {
                target
                    .as_object_mut()
                    .expect("override object")
                    .insert(key.into(), json!({}));
            }
            if !target.is_object() {
                return Err(format!(
                    "cannot migrate rules into invalid section `{target_path}`"
                ));
            }
            target = target
                .as_object_mut()
                .expect("object target")
                .entry(key)
                .or_insert_with(|| json!({}));
            if !target_path.is_empty() {
                target_path.push('.');
            }
            target_path.push_str(key);
        }
        if !target.is_object() {
            return Err(format!("cannot migrate rules into invalid group `{path}`"));
        }
        let entries = target.as_object_mut().expect("object rule group");
        let (merged, warnings) =
            canonicalize_rule_entries(std::mem::take(entries).into_iter().chain(moved), &path);
        *entries = merged;
        report.warnings.extend(warnings);
    }
    Ok(())
}

fn rename_cross_file_section(value: &mut Value, path: &str, warnings: &mut Vec<String>) {
    for &(legacy, canonical) in LEGACY_TOP_LEVEL_KEYS {
        if canonical == "cross-file" {
            rename_key(value, legacy, canonical, path, warnings);
        }
    }
}

/// Rename a top-level key on a JSON object, unless the canonical key already
/// exists (it wins). No-op for non-objects and objects without the legacy key.
fn rename_key(value: &mut Value, from: &str, to: &str, path: &str, warnings: &mut Vec<String>) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if !obj.contains_key(from) {
        return;
    }
    if obj.contains_key(to) {
        let discarded = obj.shift_remove(from).expect("existing legacy key");
        let full_path = if path.is_empty() {
            from.to_owned()
        } else {
            format!("{path}.{from}")
        };
        warnings.push(format!(
            "duplicate configuration key `{full_path}`: discarded value {discarded}; `{to}` wins"
        ));
        return;
    }
    let full_path = if path.is_empty() {
        from.to_owned()
    } else {
        format!("{path}.{from}")
    };
    warnings.push(format!(
        "deprecated configuration key `{full_path}`: use `{to}`"
    ));
    let entries = std::mem::take(obj);
    for (key, value) in entries {
        obj.insert(if key == from { to.to_string() } else { key }, value);
    }
}

/// Canonicalize the rule keys of one `linter`/`cross-file` section's `rules`
/// object (each group is an object of rule id → configuration).
fn canonicalize_section_rules(section: &mut Value, path: &str, report: &mut Migration) {
    let Some(rules) = section.get_mut("rules").and_then(Value::as_object_mut) else {
        return;
    };
    for (name, group_val) in rules.iter_mut() {
        if let Some(group) = group_val.as_object_mut() {
            canonicalize_group(group, &format!("{path}.rules.{name}"), report);
            canonicalize_group_options(group, &format!("{path}.rules.{name}"), report);
        }
    }
}

/// Rewrite every rule key in one group map to its canonical id, merging
/// collisions by keeping the more severe level.
fn canonicalize_group(group: &mut Map<String, Value>, path: &str, report: &mut Migration) {
    let mut unknown = Vec::with_capacity(group.len());
    for key in group.keys() {
        if meta_for(key).is_none() {
            report.unrecognized.push(key.clone());
            unknown.push(key.clone());
        }
    }
    for key in unknown {
        let value = group.shift_remove(&key).expect("existing unknown rule");
        report.warnings.push(format!(
            "unknown rule key `{path}.{key}`; discarded value {value}"
        ));
    }
    report.warnings.extend(canonicalize_rule_map(group, path));
}

/// Rewrite option payload keys using the shared loader alias policy.
fn canonicalize_group_options(group: &mut Map<String, Value>, path: &str, report: &mut Migration) {
    for (name, config) in group {
        if meta_for(name).is_none() {
            continue;
        }
        if let Some(options) = config.get_mut("options") {
            if options.is_null() {
                continue;
            }
            let path = format!("{path}.{name}.options");
            let warnings = canonicalize_option_keys(options, name, &path);
            report.option_key_rewrites.extend(
                warnings
                    .iter()
                    .filter(|warning| warning.starts_with("deprecated option key "))
                    .cloned(),
            );
            report.warnings.extend(warnings);
        }
    }
}

fn original_path(
    path: &[ConfigPathSegment],
    removals: &[Vec<ConfigPathSegment>],
) -> Vec<ConfigPathSegment> {
    let mut path = path.to_vec();
    for removed in removals.iter().rev() {
        let Some((ConfigPathSegment::Index(removed_index), prefix)) = removed.split_last() else {
            continue;
        };
        if path.starts_with(prefix)
            && let Some(ConfigPathSegment::Index(index)) = path.get_mut(prefix.len())
            && *index >= *removed_index
        {
            *index += 1;
        }
    }
    path
}

fn discard_at(value: &mut Value, path: &[ConfigPathSegment]) -> Option<Value> {
    let (last, parents) = path.split_last()?;
    let mut parent = value;
    for segment in parents {
        parent = match segment {
            ConfigPathSegment::Key(key) => parent.get_mut(key)?,
            ConfigPathSegment::Index(index) => parent.get_mut(*index)?,
        };
    }
    match last {
        ConfigPathSegment::Key(key) => parent.as_object_mut()?.shift_remove(key),
        ConfigPathSegment::Index(index) => {
            let array = parent.as_array_mut()?;
            (*index < array.len()).then(|| array.remove(*index))
        }
    }
}

/// Read `input`, migrate it, print a summary to stderr, and either print the
/// resulting falcon.json to stdout or write it to `output`. When `output` is
/// omitted, an upgrade writes back to `input` in place while a YAML conversion
/// defaults to `./falcon.json`. Returns 0 on success, 1 on any error.
///
/// The input is auto-detected: an existing falcon.json is *upgraded* — legacy
/// rule ids rewritten to their canonical form — while an upstream
/// `analysis_options.yaml` (the default, `./analysis_options.yaml`) is
/// *converted* into an equivalent falcon.json.
pub fn run_migrate(input: Option<PathBuf>, write: bool, output: Option<PathBuf>) -> i32 {
    let input_path = input.unwrap_or_else(|| PathBuf::from("analysis_options.yaml"));
    let content = match std::fs::read_to_string(&input_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to read {}: {}", input_path.display(), e);
            return 1;
        }
    };

    let upgrade = input_path.extension().and_then(|e| e.to_str()) == Some("json")
        || looks_like_falcon_json(&content);

    let result = if upgrade {
        migrate_existing_config(&content)
    } else {
        migrate_yaml_to_config(&content)
    };
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    if upgrade {
        eprintln!(
            "upgraded {} legacy rule id(s) and {} option key(s) to canonical form",
            result.migrated_count,
            result.option_key_rewrites.len()
        );
    } else {
        eprintln!("migrated {} rule(s)", result.migrated_count);
    }
    if !result.option_key_rewrites.is_empty() {
        eprintln!(
            "rewrote {} option key(s):",
            result.option_key_rewrites.len()
        );
        for rewrite in &result.option_key_rewrites {
            eprintln!("  - {rewrite}");
        }
    }
    for warning in &result.warnings {
        if !warning.starts_with("deprecated option key ") {
            eprintln!("warning: {warning}");
        }
    }
    if !result.unrecognized.is_empty() {
        eprintln!(
            "warning: {} unrecognized rule(s) discarded:",
            result.unrecognized.len()
        );
        for id in &result.unrecognized {
            eprintln!("  - {id}");
        }
    }

    if write {
        // In-place by default for an upgrade (write back to the falcon.json we
        // read); a YAML conversion has no falcon.json yet, so default to ./falcon.json.
        let output_path = output.unwrap_or_else(|| {
            if upgrade {
                input_path.clone()
            } else {
                PathBuf::from("falcon.json")
            }
        });
        if let Err(e) = std::fs::write(&output_path, &result.json) {
            eprintln!("error: failed to write {}: {}", output_path.display(), e);
            return 1;
        }
        eprintln!("wrote {}", output_path.display());
    } else {
        print!("{}", result.json);
    }
    0
}
