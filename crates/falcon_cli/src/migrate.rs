//! `falcon migrate`: convert a dart_code_linter / pyramid_lint
//! `analysis_options.yaml` into an equivalent biome-style `falcon.json`,
//! mirroring `biome migrate eslint/prettier`.
//!
//! The mapping table is the rule metadata itself: [`RuleSource`] carries each
//! rule's upstream id, so we invert it (upstream id → falcon `name`/`group`/
//! `cross_file`) and route every configured upstream rule to its falcon slot.

use std::collections::BTreeMap;
use std::path::PathBuf;

use falcon_config::{KNOWN_TOP_LEVEL_KEYS, LEGACY_TOP_LEVEL_KEYS, RulePlainConfiguration};
use falcon_rules::meta::{
    RULE_METADATA, RuleMeta, RuleSource, canonical_rule_name, canonicalize_rule_entries,
    canonicalize_rule_map, meta_for,
};
use falcon_rules::schema::SCHEMA_URL;
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

impl From<&Value> for RuleConfig {
    fn from(value: &Value) -> Self {
        match value {
            Value::Bool(false) => Self::Disabled,
            Value::Object(_) => Self::Enabled(Some(value.clone())),
            _ => Self::Enabled(None),
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

/// Parse one `rules:` list entry into its upstream id and falcon config. Entries
/// are a bare string, `{id: false}` (disabled), `{id: true}`/`{id: null}`
/// (enabled), or `{id: {options}}`.
fn parse_entry(entry: &Value) -> Option<(String, RuleConfig)> {
    match entry {
        Value::String(id) => Some((id.clone(), RuleConfig::Enabled(None))),
        Value::Object(map) => {
            // Single-key map keyed by the rule id; take the first key.
            let (id, val) = map.iter().next()?;
            Some((id.clone(), RuleConfig::from(val)))
        }
        _ => None,
    }
}

/// Normalize a `package:lints`-style `linter.rules` node into (id, config)
/// pairs. It is either a YAML list of ids (`- avoid_print`) or a map of
/// id → `true`/`false`/`null`/`{options}` (`avoid_print: true`).
fn parse_linter_rules(rules: &Value) -> Vec<(String, RuleConfig)> {
    match rules {
        Value::Array(entries) => entries.iter().filter_map(parse_entry).collect(),
        Value::Object(map) => map
            .iter()
            .map(|(id, val)| (id.clone(), RuleConfig::from(val)))
            .collect(),
        _ => Vec::new(),
    }
}

#[derive(Default)]
struct Migration {
    linter_groups: BTreeMap<String, Vec<(String, Value)>>,
    cross_file_groups: BTreeMap<String, Vec<(String, Value)>>,
    unrecognized: Vec<String>,
    migrated_count: usize,
}

impl Migration {
    fn place(&mut self, id: String, cfg: RuleConfig, lookup: &RuleLookup) {
        let Some(meta) = lookup.get(id.as_str()) else {
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
/// `{ "level": "warn", "options": {...} }`. Option key names are passed through
/// verbatim from the upstream config.
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
fn build_rules_object(groups: BTreeMap<String, Vec<(String, Value)>>) -> Value {
    let mut rules = Map::new();
    rules.insert("recommended".into(), json!(false));
    for (group, entries) in groups {
        if !entries.is_empty() {
            let (entries, _warnings) = canonicalize_rule_entries(entries);
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
    let (dcl, pyramid, lints) = build_lookups();

    let mut migration = Migration::default();

    // dart_code_linter rules live under `dart_code_linter.rules`; pyramid_lint is
    // a custom_lint plugin, so its rules live under `custom_lint.rules`. Both use
    // the YAML list form.
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
            migration.place(id, cfg, lookup);
        }
    }

    // Official `package:lints` / `package:flutter_lints` rules live under the
    // top-level `linter.rules` key (list or map form).
    if let Some(rules) = doc.get("linter").and_then(|s| s.get("rules")) {
        for (id, cfg) in parse_linter_rules(rules) {
            migration.place(id, cfg, &lints);
        }
    }

    let mut root = Map::new();
    root.insert("$schema".into(), json!(SCHEMA_URL));
    root.insert("linter".into(), build_rules_object(migration.linter_groups));
    // Only emit the cross-file block when a cross-file rule was actually migrated.
    if !migration.cross_file_groups.is_empty() {
        root.insert(
            "cross-file".into(),
            build_rules_object(migration.cross_file_groups),
        );
    }

    let mut json = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| format!("failed to serialize falcon.json: {e}"))?;
    json.push('\n');

    Ok(MigrationResult {
        json,
        unrecognized: migration.unrecognized,
        migrated_count: migration.migrated_count,
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
/// spellings to kebab-case — the cross-file section (`project`/`cross_file` →
/// `cross-file`, including each override's block) and `max_errors` → `max-errors`
/// — then rewrite legacy rule ids (the pre-1.0 `snake_case` ids and the removed
/// twin variants) to their canonical ids across `linter`/`cross-file` rule maps
/// and every override, preserving each rule's level and options. When a legacy id
/// and its canonical id (or two legacy twins) collide in the same group, the more
/// severe level wins.
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

    for &(legacy, canonical) in LEGACY_TOP_LEVEL_KEYS {
        rename_key(&mut root, legacy, canonical);
    }
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for ov in overrides.iter_mut() {
            rename_cross_file_section(ov);
        }
    }

    for section in ["linter", "cross-file"] {
        if let Some(s) = root.get_mut(section) {
            canonicalize_section_rules(s, &mut renamed, &mut unrecognized);
        }
    }
    if let Some(Value::Array(overrides)) = root.get_mut("overrides") {
        for ov in overrides.iter_mut() {
            for section in ["linter", "cross-file"] {
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

fn rename_cross_file_section(value: &mut Value) {
    for &(legacy, canonical) in LEGACY_TOP_LEVEL_KEYS {
        if canonical == "cross-file" {
            rename_key(value, legacy, canonical);
        }
    }
}

/// Rename a top-level key on a JSON object, unless the canonical key already
/// exists (it wins). No-op for non-objects and objects without the legacy key.
fn rename_key(value: &mut Value, from: &str, to: &str) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if !obj.contains_key(from) {
        return;
    }
    if obj.contains_key(to) {
        obj.shift_remove(from);
        return;
    }
    let entries = std::mem::take(obj);
    for (key, value) in entries {
        obj.insert(if key == from { to.to_string() } else { key }, value);
    }
}

/// Canonicalize the rule keys of one `linter`/`cross-file` section's `rules`
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
    for key in group.keys() {
        let canonical = canonical_rule_name(key);
        if meta_for(canonical).is_none() {
            if !unrecognized.contains(key) {
                unrecognized.push(key.clone());
            }
        } else if canonical != key {
            *renamed += 1;
        }
    }
    let _warnings = canonicalize_rule_map(group);
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
            "upgraded {} legacy rule id(s) to canonical form",
            result.migrated_count
        );
    } else {
        eprintln!("migrated {} rule(s)", result.migrated_count);
    }
    if !result.unrecognized.is_empty() {
        let description = if upgrade {
            "rule id(s) matched no known rule (left unchanged)"
        } else {
            "unrecognized upstream rule(s) skipped"
        };
        eprintln!("warning: {} {description}:", result.unrecognized.len());
        for id in &result.unrecognized {
            eprintln!("  - {id}");
        }
    }
    if !upgrade {
        eprintln!("note: rule option keys are passed through verbatim and may need manual review");
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
