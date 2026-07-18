//! `falcon migrate`: convert a dart_code_linter / pyramid_lint
//! `analysis_options.yaml` into an equivalent biome-style `falcon.json`,
//! mirroring `biome migrate eslint/prettier`.
//!
//! The mapping table is the rule metadata itself: [`RuleSource`] carries each
//! rule's upstream id, so we invert it (upstream id → falcon `name`/`group`/
//! `project`) and route every configured upstream rule to its falcon slot.

use std::collections::BTreeMap;
use std::path::PathBuf;

use falcon_rules::meta::{RULE_METADATA, RuleMeta, RuleSource};
use falcon_rules::schema::SCHEMA_URL;
use serde_json::{Map, Value, json};

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

/// Read `input` (default `./analysis_options.yaml`), migrate it, print a summary
/// to stderr, and either print the JSON to stdout or write it to `output`
/// (default `./falcon.json`). Returns 0 on success, 1 on any error.
pub fn run_migrate(input: Option<PathBuf>, write: bool, output: Option<PathBuf>) -> i32 {
    let input_path = input.unwrap_or_else(|| PathBuf::from("analysis_options.yaml"));
    let content = match std::fs::read_to_string(&input_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to read {}: {}", input_path.display(), e);
            return 1;
        }
    };

    let result = match migrate_yaml_to_config(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

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
