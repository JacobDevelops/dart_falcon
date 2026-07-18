//! Tests for the pure `migrate_yaml_to_config` core: upstream-id mapping,
//! options passthrough, disabled entries, unrecognized rules, and round-trip
//! back through `FalconConfig`.

use falcon_cli::migrate_yaml_to_config;
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
    // (a project-level correctness rule) — upstream id differs from falcon name.
    let yaml = "\
dart_code_linter:
  rules:
    - check-unused-files
";
    let (value, unrecognized, count) = parse(yaml);
    assert!(unrecognized.is_empty());
    assert_eq!(count, 1);
    assert_eq!(
        value["project"]["rules"]["correctness"]["unused-files"],
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
    let cfg = &value["linter"]["rules"]["complexity"]["max_lines_for_file"];
    assert_eq!(cfg["level"], Value::String("warn".into()));
    assert_eq!(cfg["options"]["max_lines"], Value::from(200));
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
    assert_eq!(config.project.rules.recommended, Some(false));
}
