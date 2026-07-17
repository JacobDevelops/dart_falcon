//! Tests for the pure `migrate_yaml_to_config` core: upstream-id mapping,
//! options passthrough, disabled entries, unrecognized rules, and round-trip
//! back through `FalconConfig`.

use falcon_cli::{migrate_existing_config, migrate_yaml_to_config};
use serde_json::Value;

/// Parse the emitted JSON so assertions target structure, not whitespace.
fn parse(yaml: &str) -> (Value, Vec<String>, usize) {
    let result = migrate_yaml_to_config(yaml).expect("migration succeeds");
    let value: Value = serde_json::from_str(&result.json).expect("emitted JSON parses");
    (value, result.unrecognized, result.migrated_count)
}

#[test]
fn dcl_rule_with_differing_name_maps_to_falcon_name_and_group() {
    // dart_code_linter's `check-unused-files` maps to falcon `unused-files`
    // (a cross-file correctness rule) — upstream id differs from falcon name.
    let yaml = "\
dart_code_linter:
  rules:
    - check-unused-files
";
    let (value, unrecognized, count) = parse(yaml);
    assert!(unrecognized.is_empty());
    assert_eq!(count, 1);
    assert_eq!(
        value["cross-file"]["rules"]["correctness"]["unused-files"],
        Value::String("warn".into())
    );
}

#[test]
fn pyramid_rule_with_options_maps_to_level_and_options() {
    let yaml = "\
custom_lint:
  rules:
    - max_lines_for_file:
        max_lines: 200
";
    let (value, unrecognized, count) = parse(yaml);
    assert!(unrecognized.is_empty());
    assert_eq!(count, 1);
    // The upstream pyramid id keeps working; it maps to the canonical falcon id.
    let cfg = &value["linter"]["rules"]["complexity"]["max-lines-for-file"];
    assert_eq!(cfg["level"], Value::String("warn".into()));
    assert_eq!(cfg["options"]["max_lines"], Value::from(200));
}

#[test]
fn merged_twin_upstream_ids_map_to_surviving_rule() {
    // pyramid_lint's `no_empty_block`, `avoid_empty_blocks`, `no_magic_number`,
    // and `avoid_unused_parameters` were merged into their dart_code_linter
    // counterparts; their upstream ids must still map to the surviving rule.
    let yaml = "\
custom_lint:
  rules:
    - avoid_empty_blocks
    - no_magic_number
    - avoid_unused_parameters
";
    let (value, unrecognized, count) = parse(yaml);
    assert!(unrecognized.is_empty(), "unexpected: {unrecognized:?}");
    assert_eq!(count, 3);
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["no-empty-block"],
        Value::String("warn".into())
    );
    assert_eq!(
        value["linter"]["rules"]["style"]["no-magic-number"],
        Value::String("warn".into())
    );
    assert_eq!(
        value["linter"]["rules"]["correctness"]["avoid-unused-parameters"],
        Value::String("warn".into())
    );
}

#[test]
fn disabled_entry_emits_off() {
    let yaml = "\
dart_code_linter:
  rules:
    - no-empty-block: false
";
    let (value, _unrecognized, count) = parse(yaml);
    assert_eq!(count, 1);
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["no-empty-block"],
        Value::String("off".into())
    );
}

#[test]
fn unknown_rule_is_reported_and_not_emitted() {
    let yaml = "\
dart_code_linter:
  rules:
    - this-rule-does-not-exist
";
    let (value, unrecognized, count) = parse(yaml);
    assert_eq!(count, 0);
    assert_eq!(unrecognized, vec!["this-rule-does-not-exist".to_string()]);
    // No linter groups were populated, so `recommended` is the only key.
    let rules = value["linter"]["rules"].as_object().expect("rules object");
    assert_eq!(rules.len(), 1);
    assert!(rules.contains_key("recommended"));
}

#[test]
fn recommended_is_false_in_emitted_config() {
    let yaml = "\
dart_code_linter:
  rules:
    - avoid-non-null-assertion
";
    let (value, _unrecognized, _count) = parse(yaml);
    assert_eq!(value["linter"]["rules"]["recommended"], Value::Bool(false));
}

#[test]
fn emitted_json_round_trips_through_falcon_config() {
    let yaml = "\
dart_code_linter:
  rules:
    - avoid-non-null-assertion
    - no-empty-block: false
    - check-unused-files
custom_lint:
  rules:
    - max_lines_for_file:
        max_lines: 200
";
    let result = migrate_yaml_to_config(yaml).expect("migration succeeds");
    // Round-trip sanity: the emitted config deserializes without error.
    let config: falcon_config::FalconConfig =
        serde_json::from_str(&result.json).expect("emitted JSON is a valid FalconConfig");
    assert_eq!(config.linter.rules.recommended, Some(false));
    assert_eq!(config.cross_file.rules.recommended, Some(false));
}

#[test]
fn upgrade_rewrites_legacy_ids_in_existing_falcon_json() {
    // A pre-1.0 falcon.json using snake_case ids is upgraded to canonical
    // kebab-case ids, preserving levels and options.
    let input = r#"{
      "linter": {
        "rules": {
          "complexity": {
            "max_lines_for_file": { "level": "error", "options": { "max_lines": 300 } }
          },
          "style": { "class_members_ordering": "warn" }
        }
      }
    }"#;
    let result = migrate_existing_config(input).expect("upgrade succeeds");
    assert_eq!(result.migrated_count, 2);
    let value: Value = serde_json::from_str(&result.json).expect("valid JSON");
    let complexity = &value["linter"]["rules"]["complexity"];
    assert!(complexity.get("max_lines_for_file").is_none());
    assert_eq!(complexity["max-lines-for-file"]["level"], "error");
    assert_eq!(
        complexity["max-lines-for-file"]["options"]["max_lines"],
        300
    );
    assert_eq!(
        value["linter"]["rules"]["style"]["class-members-ordering"],
        Value::String("warn".into())
    );
}

#[test]
fn upgrade_rewrites_legacy_project_section_to_cross_file() {
    // A pre-rename falcon.json using the top-level `project` section (and an
    // override's `project` block) is rewritten to `cross_file`, preserving rules.
    let input = r#"{
      "project": {
        "rules": { "correctness": { "unused-files": "error" } }
      },
      "overrides": [
        { "includes": ["gen/**"], "project": { "enabled": false } }
      ]
    }"#;
    let result = migrate_existing_config(input).expect("upgrade succeeds");
    let value: Value = serde_json::from_str(&result.json).expect("valid JSON");
    // The legacy key is gone; the canonical key carries the rules.
    assert!(value.get("project").is_none());
    assert_eq!(
        value["cross-file"]["rules"]["correctness"]["unused-files"],
        Value::String("error".into())
    );
    // The override's block is renamed too.
    assert!(value["overrides"][0].get("project").is_none());
    assert_eq!(value["overrides"][0]["cross-file"]["enabled"], Value::Bool(false));
    // The upgraded config deserializes cleanly under the new key.
    let config: falcon_config::FalconConfig =
        serde_json::from_str(&result.json).expect("upgraded JSON is a valid FalconConfig");
    assert!(config.cross_file.enabled);
}

#[test]
fn upgrade_merges_duplicate_twins_keeping_more_severe_level() {
    // A config that configured both twin variants collapses to the surviving
    // rule; the more severe level wins.
    let input = r#"{
      "linter": {
        "rules": {
          "suspicious": {
            "no-empty-block": "warn",
            "no_empty_block": "error",
            "avoid_empty_blocks": "off"
          }
        }
      }
    }"#;
    let result = migrate_existing_config(input).expect("upgrade succeeds");
    let value: Value = serde_json::from_str(&result.json).expect("valid JSON");
    let suspicious = value["linter"]["rules"]["suspicious"]
        .as_object()
        .expect("group object");
    // All three collapse into the single canonical key at the most severe level.
    assert_eq!(suspicious.len(), 1);
    assert_eq!(suspicious["no-empty-block"], Value::String("error".into()));
}

#[test]
fn official_lints_rules_list_maps_to_falcon_ids() {
    // package:lints / flutter_lints rules live under the top-level `linter.rules`
    // key (list form). Official snake_case ids map to falcon kebab ids, camelCase
    // humps split, and a `- id: false` entry disables the rule.
    let yaml = "\
linter:
  rules:
    - avoid_print
    - prefer_iterable_whereType
    - unnecessary_new: false
";
    let (value, unrecognized, count) = parse(yaml);
    assert!(unrecognized.is_empty(), "unexpected: {unrecognized:?}");
    assert_eq!(count, 3);
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["avoid-print"],
        Value::String("warn".into())
    );
    assert_eq!(
        value["linter"]["rules"]["style"]["prefer-iterable-where-type"],
        Value::String("warn".into())
    );
    assert_eq!(
        value["linter"]["rules"]["style"]["unnecessary-new"],
        Value::String("off".into())
    );
}

#[test]
fn official_lints_rules_map_form_maps_to_falcon_ids() {
    // The map form (`id: true` / `id: false`) is equally valid in
    // analysis_options.yaml and must map through the same lints lookup.
    let yaml = "\
linter:
  rules:
    avoid_print: true
    prefer_is_empty: false
";
    let (value, unrecognized, count) = parse(yaml);
    assert!(unrecognized.is_empty(), "unexpected: {unrecognized:?}");
    assert_eq!(count, 2);
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["avoid-print"],
        Value::String("warn".into())
    );
    assert_eq!(
        value["linter"]["rules"]["style"]["prefer-is-empty"],
        Value::String("off".into())
    );
}

#[test]
fn upgrade_leaves_canonical_config_unchanged() {
    let input = r#"{
      "linter": { "rules": { "style": { "no-magic-number": "warn" } } }
    }"#;
    let result = migrate_existing_config(input).expect("upgrade succeeds");
    assert_eq!(result.migrated_count, 0);
    assert!(result.unrecognized.is_empty());
    let value: Value = serde_json::from_str(&result.json).expect("valid JSON");
    assert_eq!(
        value["linter"]["rules"]["style"]["no-magic-number"],
        Value::String("warn".into())
    );
}
