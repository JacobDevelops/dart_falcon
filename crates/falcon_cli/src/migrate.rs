//! `falcon migrate`: convert a dart_code_linter / pyramid_lint
//! `analysis_options.yaml` into an equivalent biome-style `falcon.json`,
//! mirroring `biome migrate eslint/prettier`.
//!
//! The mapping table is the rule metadata itself: [`RuleSource`] carries each
//! rule's upstream id, so we invert it (upstream id → falcon `name`/`group`/
//! `project`) and route every configured upstream rule to its falcon slot.

use std::collections::BTreeMap;
use std::path::PathBuf;

use falcon_rules::meta::{RULE_METADATA, RuleMeta, RuleSource, canonical_rule_name, meta_for};
use falcon_rules::schema::SCHEMA_URL;
use serde_json::{Map, Value, json};

/// Upstream pyramid_lint ids whose rules were merged into a surviving
/// dart_code_linter rule during twin unification. Their metadata now carries the
/// dart_code_linter `source`, so [`build_lookups`] would otherwise drop these
/// upstream ids; they are re-added to the pyramid lookup so `falcon migrate`
/// still maps a `custom_lint` config that used them.
const MERGED_PYRAMID_UPSTREAM: &[(&str, &str)] =
    &[("avoid_unused_parameters", "avoid-unused-parameters")];

/// Outcome of a migration: the generated falcon.json plus a report of what was
/// and was not mapped.
pub struct MigrationResult {
    /// Pretty-printed falcon.json with a trailing newline.
    pub json: String,
    /// Upstream rule ids that matched no falcon rule (reported, not emitted).
    pub unrecognized: Vec<String>,
    /// Number of upstream rules successfully mapped into the output.
    pub migrated_count: usize,
}

/// The falcon config for one upstream entry, before JSON serialization.
enum RuleConfig {
    Disabled,
    Enabled(Option<Value>),
}

/// Invert [`RULE_METADATA`] into (dcl-id → meta, pyramid-id → meta) lookups.
fn build_lookups() -> (
    BTreeMap<&'static str, &'static RuleMeta>,
    BTreeMap<&'static str, &'static RuleMeta>,
) {
    let mut dcl = BTreeMap::new();
    let mut pyramid = BTreeMap::new();
    for meta in RULE_METADATA {
        match meta.source {
            RuleSource::DartCodeLinter(id) => {
                dcl.insert(id, meta);
            }
            RuleSource::PyramidLint(id) => {
                pyramid.insert(id, meta);
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
    (dcl, pyramid)
}

/// Parse one `rules:` list entry into its upstream id and falcon config. Entries
/// are a bare string, `{id: false}` (disabled), `{id: true}`/`{id: null}`
/// (enabled), or `{id: {options}}`.
fn parse_entry(entry: &Value) -> Option<(String, RuleConfig)> {
    match entry {
        Value::String(id) => Some((id.clone(), RuleConfig::Enabled(None))),
        Value::Object(map) => {
            // Single-key map keyed by the rule id; take the first key.
            let (id, val) = map.iter().next()?;
            let cfg = match val {
                Value::Bool(false) => RuleConfig::Disabled,
                Value::Object(opts) => RuleConfig::Enabled(Some(Value::Object(opts.clone()))),
                // Bare `- id:`, `- id: true`, or an unexpected scalar: enable it.
                _ => RuleConfig::Enabled(None),
            };
            Some((id.clone(), cfg))
        }
        _ => None,
    }
}

/// Serialize a [`RuleConfig`] to its falcon.json value: `"off"`, `"warn"`, or
/// `{ "level": "warn", "options": {...} }`. Option key names are passed through
/// verbatim from the upstream config.
fn config_to_json(cfg: RuleConfig) -> Value {
    match cfg {
        RuleConfig::Disabled => json!("off"),
        RuleConfig::Enabled(None) => json!("warn"),
        RuleConfig::Enabled(Some(options)) => json!({ "level": "warn", "options": options }),
    }
}

/// Assemble a section's `rules` object: `recommended: false` plus each non-empty
/// group. Groups are omitted when they contain no migrated rules.
fn build_rules_object(groups: BTreeMap<String, Map<String, Value>>) -> Value {
    let mut rules = Map::new();
    rules.insert("recommended".into(), json!(false));
    for (group, entries) in groups {
        if !entries.is_empty() {
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
/// serialized.
pub fn migrate_yaml_to_config(yaml: &str) -> Result<MigrationResult, String> {
    let doc: Value =
        serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse YAML: {e}"))?;
    let (dcl, pyramid) = build_lookups();

    let mut linter_groups: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut project_groups: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut unrecognized = Vec::new();
    let mut migrated_count = 0usize;

    // dart_code_linter rules live under `dart_code_linter.rules`; pyramid_lint is
    // a custom_lint plugin, so its rules live under `custom_lint.rules`.
    let sections = [("dart_code_linter", &dcl), ("custom_lint", &pyramid)];
    for (section, lookup) in sections {
        let Some(rules) = doc
            .get(section)
            .and_then(|s| s.get("rules"))
            .and_then(|r| r.as_array())
        else {
            continue;
        };
        for entry in rules {
            let Some((id, cfg)) = parse_entry(entry) else {
                continue;
            };
            let Some(meta) = lookup.get(id.as_str()) else {
                unrecognized.push(id);
                continue;
            };
            let target = if meta.project {
                &mut project_groups
            } else {
                &mut linter_groups
            };
            target
                .entry(meta.group.to_string())
                .or_default()
                .insert(meta.name.to_string(), config_to_json(cfg));
            migrated_count += 1;
        }
    }

    let mut root = Map::new();
    root.insert("$schema".into(), json!(SCHEMA_URL));
    root.insert("linter".into(), build_rules_object(linter_groups));
    // Only emit the project block when a project rule was actually migrated.
    if !project_groups.is_empty() {
        root.insert("project".into(), build_rules_object(project_groups));
    }

    let mut json = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| format!("failed to serialize falcon.json: {e}"))?;
    json.push('\n');

    Ok(MigrationResult {
        json,
        unrecognized,
        migrated_count,
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
            ["linter", "project", "files", "overrides", "$schema"]
                .iter()
                .any(|k| obj.contains_key(*k))
        })
}

/// Upgrade an existing falcon.json in place: rewrite legacy rule ids (the
/// pre-1.0 `snake_case` ids and the removed twin variants) to their canonical
/// ids across `linter`/`project` rule maps and every override, preserving each
/// rule's level and options. When a legacy id and its canonical id (or two
/// legacy twins) collide in the same group, the more severe level wins.
///
/// # Errors
///
/// Returns an error string if the input is not valid JSON or cannot be
/// re-serialized.
pub fn migrate_existing_config(json: &str) -> Result<MigrationResult, String> {
    let mut root: Value =
        serde_json::from_str(json).map_err(|e| format!("failed to parse falcon.json: {e}"))?;
    let mut renamed = 0usize;
    let mut unrecognized = Vec::new();

    for section in ["linter", "project"] {
        if let Some(s) = root.get_mut(section) {
            canonicalize_section_rules(s, &mut renamed, &mut unrecognized);
        }
    }
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for ov in overrides.iter_mut() {
            for section in ["linter", "project"] {
                if let Some(s) = ov.get_mut(section) {
                    canonicalize_section_rules(s, &mut renamed, &mut unrecognized);
                }
            }
        }
    }

    let mut out = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("failed to serialize falcon.json: {e}"))?;
    out.push('\n');
    Ok(MigrationResult {
        json: out,
        unrecognized,
        migrated_count: renamed,
    })
}

/// Canonicalize the rule keys of one `linter`/`project` section's `rules`
/// object (each group is an object of rule id → configuration).
fn canonicalize_section_rules(
    section: &mut Value,
    renamed: &mut usize,
    unrecognized: &mut Vec<String>,
) {
    let Some(rules) = section.get_mut("rules").and_then(Value::as_object_mut) else {
        return;
    };
    for group_val in rules.values_mut() {
        if let Some(group) = group_val.as_object_mut() {
            canonicalize_group(group, renamed, unrecognized);
        }
    }
}

/// Rewrite every rule key in one group map to its canonical id, merging
/// collisions by keeping the more severe level.
fn canonicalize_group(
    group: &mut Map<String, Value>,
    renamed: &mut usize,
    unrecognized: &mut Vec<String>,
) {
    for (key, value) in std::mem::take(group) {
        let canonical = canonical_rule_name(&key).to_string();
        if meta_for(&canonical).is_none() {
            if !unrecognized.contains(&key) {
                unrecognized.push(key.clone());
            }
            // Unknown key: keep it verbatim (it was never renamed).
            group.entry(key).or_insert(value);
            continue;
        }
        let renamed_key = canonical != key;
        match group.get(&canonical) {
            Some(existing) if level_rank(&value) <= level_rank(existing) => {}
            _ => {
                group.insert(canonical, value);
                // Count only entries actually written under a new id.
                if renamed_key {
                    *renamed += 1;
                }
            }
        }
    }
}

/// Severity ordering for merging duplicate twin entries: higher wins.
fn level_rank(value: &Value) -> u8 {
    let level = match value {
        Value::String(s) => s.as_str(),
        Value::Object(o) => o.get("level").and_then(Value::as_str).unwrap_or(""),
        _ => "",
    };
    match level {
        "error" => 4,
        "warn" | "on" => 3,
        "info" => 2,
        "off" => 1,
        _ => 0,
    }
}

/// Read `input`, migrate it, print a summary to stderr, and either print the
/// resulting falcon.json to stdout or write it to `output` (default
/// `./falcon.json`). Returns 0 on success, 1 on any error.
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
            "upgraded {} legacy rule id(s) to canonical form",
            result.migrated_count
        );
        if !result.unrecognized.is_empty() {
            eprintln!(
                "warning: {} rule id(s) matched no known rule (left unchanged):",
                result.unrecognized.len()
            );
            for id in &result.unrecognized {
                eprintln!("  - {id}");
            }
        }
    } else {
        eprintln!("migrated {} rule(s)", result.migrated_count);
        if !result.unrecognized.is_empty() {
            eprintln!(
                "warning: {} unrecognized upstream rule(s) skipped:",
                result.unrecognized.len()
            );
            for id in &result.unrecognized {
                eprintln!("  - {id}");
            }
        }
        eprintln!("note: rule option keys are passed through verbatim and may need manual review");
    }

    if write {
        let output_path = output.unwrap_or_else(|| PathBuf::from("falcon.json"));
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
