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
    KNOWN_TOP_LEVEL_KEYS, LEGACY_TOP_LEVEL_KEYS, RuleOptions, RulePlainConfiguration,
    deserialize_config_value,
};
use falcon_rules::meta::{
    RULE_METADATA, RuleMeta, RuleSource, canonical_rule_name, canonicalize_rule_entries,
    canonicalize_rule_map, meta_for,
};
use falcon_rules::schema::SCHEMA_URL;
use falcon_rules::{canonicalize_loaded_config, canonicalize_option_keys, validate_rule_options};
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
        let Some(meta) = lookup.get(id.as_str()) else {
            let path = match index {
                Some(index) => format!("{source}.rules[{index}].{id}"),
                None => format!("{source}.rules.{id}"),
            };
            self.warnings.push(format!(
                "unknown upstream rule `{path}`; discarded value {value}"
            ));
            self.unrecognized.push(id);
            return;
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

    let json = serialize_config(Value::Object(root), &mut migration.warnings)?;

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
    if let Some(object) = root.as_object_mut() {
        for key in ["rules", "exclude_patterns", "severity_override"] {
            if let Some(value) = object.shift_remove(key) {
                report.warnings.push(format!(
                    "unsupported legacy flat configuration `{key}`; discarded value {value}"
                ));
            }
        }
    }

    for &(legacy, canonical) in LEGACY_TOP_LEVEL_KEYS {
        rename_key(&mut root, legacy, canonical, "", &mut report.warnings);
    }
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for (index, ov) in overrides.iter_mut().enumerate() {
            rename_cross_file_section(ov, &format!("overrides[{index}]"), &mut report.warnings);
        }
    }

    for section in ["linter", "cross-file"] {
        if let Some(s) = root.get_mut(section) {
            canonicalize_section_rules(s, section, &mut report);
        }
    }
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for (index, ov) in overrides.iter_mut().enumerate() {
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

    let out = serialize_config(root, &mut report.warnings)?;
    Ok(MigrationResult {
        json: out,
        unrecognized: report.unrecognized,
        migrated_count: report.migrated_count,
        warnings: report.warnings,
        option_key_rewrites: report.option_key_rewrites,
    })
}

/// Repair invalid entries using the canonical loader before serializing.
fn serialize_config(mut value: Value, warnings: &mut Vec<String>) -> Result<String, String> {
    let mut removals = Vec::new();
    loop {
        let loaded = deserialize_config_value(&value, Path::new("falcon.json"))
            .and_then(canonicalize_loaded_config);
        match loaded {
            Ok(loaded) => {
                for warning in loaded.warnings {
                    if !warnings.contains(&warning) {
                        warnings.push(warning);
                    }
                }
                break;
            }
            Err(error) => {
                let path = error.0.split('`').nth(1).unwrap_or("");
                let original_path = original_option_path(path, &removals);
                let suffix = if path.is_empty() || path == "." {
                    String::new()
                } else {
                    format!(".{path}")
                };
                let (discarded_path, discarded, array_entry) =
                    discard_option(&mut value, &suffix, "")
                        .unwrap_or_else(|| (String::new(), std::mem::take(&mut value), false));
                let original_discarded_path = original_option_path(&discarded_path, &removals);
                warnings.push(format!(
                    "{}; discarded value {discarded} at `{}`",
                    error.0.replace(path, &original_path),
                    if original_discarded_path.is_empty() {
                        "$"
                    } else {
                        &original_discarded_path
                    }
                ));
                if array_entry {
                    let (prefix, index) =
                        discarded_path.rsplit_once('[').expect("array entry path");
                    removals.push((
                        prefix.to_owned(),
                        index
                            .trim_end_matches(']')
                            .parse::<usize>()
                            .expect("array entry index"),
                    ));
                }
                if value.is_null() {
                    value = Value::Object(Map::new());
                }
            }
        }
    }
    let mut json = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("failed to serialize falcon.json: {e}"))?;
    json.push('\n');
    Ok(json)
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
        let canonical = canonical_rule_name(key);
        if meta_for(key).is_none() {
            report.unrecognized.push(key.clone());
            unknown.push(key.clone());
        } else if canonical != key {
            report.migrated_count += 1;
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
            // Validation consumes a temporary wrapper, not a copy of the payload.
            let mut data: RuleOptions = serde_json::from_value(std::mem::take(options))
                .expect("any JSON value deserializes as a rule options payload");
            let mut removals = Vec::new();
            let mut optionless = false;
            while let Err(error) = validate_rule_options(name, &data, &path) {
                optionless |= error.0.contains("this rule has no options");
                let invalid_path = error.0.split('`').nth(1).unwrap_or(&path);
                let suffix = invalid_path.strip_prefix(&path).unwrap_or("");
                let original_path = original_option_path(invalid_path, &removals);
                let (discarded_path, discarded, array_entry) =
                    discard_option(data.value_mut(), suffix, &path)
                        .unwrap_or_else(|| (path.clone(), std::mem::take(data.value_mut()), false));
                let original_discarded_path = original_option_path(&discarded_path, &removals);
                report.warnings.push(format!(
                    "{}; discarded value {discarded} at `{original_discarded_path}`",
                    error.0.replace(invalid_path, &original_path)
                ));
                if array_entry {
                    let (prefix, index) =
                        discarded_path.rsplit_once('[').expect("array entry path");
                    removals.push((
                        prefix.to_owned(),
                        index
                            .trim_end_matches(']')
                            .parse::<usize>()
                            .expect("array entry index"),
                    ));
                }
                if data.value().is_null() {
                    break;
                }
            }
            *options = std::mem::take(data.value_mut());
            if options.is_null() || optionless {
                config
                    .as_object_mut()
                    .expect("options belongs to an object")
                    .shift_remove("options");
            }
        }
    }
}

fn original_option_path(path: &str, removals: &[(String, usize)]) -> String {
    let mut path = path.to_owned();
    for (prefix, removed) in removals.iter().rev() {
        let Some(suffix) = path
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_prefix('['))
        else {
            continue;
        };
        let Some((index, rest)) = suffix.split_once(']') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        if index >= *removed {
            path = format!("{prefix}[{}]{rest}", index + 1);
        }
    }
    path
}

fn discard_option(value: &mut Value, suffix: &str, path: &str) -> Option<(String, Value, bool)> {
    if suffix.is_empty() {
        return Some((path.to_owned(), std::mem::take(value), false));
    }
    match value {
        Value::Object(entries) => {
            let suffix = suffix.strip_prefix('.')?;
            let key = entries
                .keys()
                .filter(|key| {
                    suffix == key.as_str()
                        || suffix
                            .strip_prefix(key.as_str())
                            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
                })
                .max_by_key(|key| key.len())?
                .clone();
            let rest = suffix.strip_prefix(&key)?;
            let path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            if rest.is_empty() {
                Some((path, entries.shift_remove(&key)?, false))
            } else {
                discard_option(entries.get_mut(&key)?, rest, &path)
                    .or_else(|| Some((path, entries.shift_remove(&key)?, false)))
            }
        }
        Value::Array(entries) => {
            let (index, rest) = suffix.strip_prefix('[')?.split_once(']')?;
            let index = index.parse::<usize>().ok()?;
            if index >= entries.len() {
                return None;
            }
            let path = format!("{path}[{index}]");
            if rest.is_empty() {
                Some((path, entries.remove(index), true))
            } else {
                discard_option(&mut entries[index], rest, &path)
                    .or_else(|| Some((path, entries.remove(index), true)))
            }
        }
        _ => None,
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
