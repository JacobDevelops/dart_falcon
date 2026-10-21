//! Tests for the pure `migrate_yaml_to_config` core: upstream-id mapping,
//! option-key canonicalization, disabled entries, unrecognized rules, and round-trip
//! back through `FalconConfig`.

use falcon_cli::{migrate_existing_config, migrate_yaml_to_config, run_migrate};
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
fn secure_pubspec_urls_migrates_to_cross_file_security() {
    let (value, unrecognized, count) = parse("linter:\n  rules:\n    - secure_pubspec_urls\n");
    assert!(unrecognized.is_empty());
    assert_eq!(count, 1);
    assert_eq!(
        value["cross-file"]["rules"]["security"]["secure-pubspec-urls"],
        Value::String("warn".into())
    );
    assert!(
        value["cross-file"]["rules"]["suspicious"]
            .get("secure-pubspec-urls")
            .is_none()
    );
    assert!(
        value["linter"]["rules"]["security"]
            .get("secure-pubspec-urls")
            .is_none()
    );
    let _: falcon_config::FalconConfig = serde_json::from_value(value).unwrap();
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
    assert_eq!(cfg["options"]["max-lines"], Value::from(200));
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
    let config = falcon_rules::canonicalize_loaded_config(falcon_config::LoadedConfig {
        config: serde_json::from_str(&result.json).expect("emitted JSON is a valid FalconConfig"),
        warnings: Vec::new(),
    })
    .expect("emitted config passes rules loader validation")
    .config;
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
        complexity["max-lines-for-file"]["options"]["max-lines"],
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
    assert_eq!(
        value["overrides"][0]["cross-file"]["enabled"],
        Value::Bool(false)
    );
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

#[test]
fn upgrade_legacy_section_collisions_keep_first_and_remove_all_aliases() {
    for canonical in [false, true] {
        let mut input = serde_json::json!({
            "project": {"enabled": false},
            "cross_file": {"enabled": true},
            "max_errors": 3,
            "overrides": [{"project": {"enabled": false}, "cross_file": {"enabled": true}}]
        });
        if canonical {
            input["cross-file"] = serde_json::json!({"enabled": true});
            input["max-errors"] = serde_json::json!(5);
            input["overrides"][0]["cross-file"] = serde_json::json!({"enabled": true});
        }
        let result = migrate_existing_config(&input.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        assert!(value.get("project").is_none());
        assert!(value.get("cross_file").is_none());
        assert!(value.get("max_errors").is_none());
        assert_eq!(value["cross-file"]["enabled"], canonical);
        assert_eq!(value["max-errors"], if canonical { 5 } else { 3 });
        assert!(value["overrides"][0].get("project").is_none());
        assert!(value["overrides"][0].get("cross_file").is_none());
        assert_eq!(value["overrides"][0]["cross-file"]["enabled"], canonical);
        let _: falcon_config::FalconConfig = serde_json::from_str(&result.json).unwrap();
    }
}

#[test]
fn upgrade_collision_levels_preserve_unknown_off_and_tied_options() {
    for (legacy, canonical, expected) in [
        (
            serde_json::json!("off"),
            serde_json::json!("invalid"),
            serde_json::json!("off"),
        ),
        (
            serde_json::json!("info"),
            serde_json::json!("off"),
            serde_json::json!("info"),
        ),
        (
            serde_json::json!("on"),
            serde_json::json!("info"),
            serde_json::json!("on"),
        ),
        (
            serde_json::json!("error"),
            serde_json::json!("warn"),
            serde_json::json!("error"),
        ),
        (
            serde_json::json!({"level":"on","options":{}}),
            serde_json::json!("warn"),
            serde_json::json!("warn"),
        ),
    ] {
        let input = serde_json::json!({
            "linter": {"rules": {"suspicious": {
                "no_empty_block": legacy, "no-empty-block": canonical
            }}}
        });
        let result = migrate_existing_config(&input.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        assert_eq!(
            value["linter"]["rules"]["suspicious"]["no-empty-block"],
            expected
        );
    }
}

#[test]
fn run_migrate_upgrades_in_place_without_reordering_keys() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("falcon.json");
    std::fs::write(
        &input,
        r#"{
  "max_errors": 4,
  "project": {"enabled": false},
  "linter": {
    "rules": {
      "suspicious": {
        "no_empty_block": "error",
        "no_self_comparisons": "warn"
      },
      "recommended": false
    },
    "enabled": true
  },
  "$schema": "https://example.com/schema.json"
}"#,
    )
    .unwrap();
    assert_eq!(run_migrate(Some(input.clone()), true, None), 0);
    assert_eq!(
        std::fs::read_to_string(&input).unwrap(),
        r#"{
  "max-errors": 4,
  "cross-file": {
    "enabled": false
  },
  "linter": {
    "rules": {
      "suspicious": {
        "no-empty-block": "error",
        "no-self-comparisons": "warn"
      },
      "recommended": false
    },
    "enabled": true
  },
  "$schema": "https://example.com/schema.json"
}
"#
    );
}

#[test]
fn run_migrate_converts_yaml_to_explicit_output_without_changing_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("analysis_options.yaml");
    let output = dir.path().join("converted.json");
    let yaml = "linter:\n  rules:\n    - avoid_print\n";
    std::fs::write(&input, yaml).unwrap();
    assert_eq!(
        run_migrate(Some(input.clone()), true, Some(output.clone())),
        0
    );
    assert_eq!(std::fs::read_to_string(input).unwrap(), yaml);
    let loaded = falcon_rules::load_config(&output).expect("written migration output loads");
    assert!(loaded.warnings.is_empty());
    let value: Value = serde_json::from_str(&std::fs::read_to_string(output).unwrap()).unwrap();
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["avoid-print"],
        "warn"
    );
}

#[test]
fn run_migrate_detects_json_content_and_dry_run_does_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("config.txt");
    let output = dir.path().join("ignored.json");
    let json = r#"{"linter":{"rules":{"suspicious":{"no_empty_block":"warn"}}}}"#;
    std::fs::write(&input, json).unwrap();
    assert_eq!(
        run_migrate(Some(input.clone()), false, Some(output.clone())),
        0
    );
    assert_eq!(std::fs::read_to_string(&input).unwrap(), json);
    assert!(!output.exists());
    assert_eq!(run_migrate(Some(input.clone()), true, None), 0);
    let value: Value = serde_json::from_str(&std::fs::read_to_string(input).unwrap()).unwrap();
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["no-empty-block"],
        "warn"
    );
}

#[test]
fn run_migrate_reports_read_parse_and_write_failures() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("falcon.json");
    assert_eq!(run_migrate(Some(input.clone()), true, None), 1);
    for (name, malformed) in [("falcon.json", "{"), ("analysis_options.yaml", "linter: [")] {
        let path = dir.path().join(name);
        std::fs::write(&path, malformed).unwrap();
        assert_eq!(run_migrate(Some(path.clone()), true, None), 1);
        assert_eq!(std::fs::read_to_string(path).unwrap(), malformed);
    }
    std::fs::write(&input, r#"{"linter":{}}"#).unwrap();
    assert_eq!(
        run_migrate(Some(input), true, Some(dir.path().to_path_buf())),
        1
    );
}

#[test]
fn upgrade_equal_rank_prefers_canonical_options_in_either_input_order() {
    for entries in [
        r#""max_lines_for_file":{"level":"on","options":{"max-lines":20}},"max-lines-for-file":{"level":"warn","options":{"max-lines":30}}"#,
        r#""max-lines-for-file":{"level":"warn","options":{"max-lines":30}},"max_lines_for_file":{"level":"on","options":{"max-lines":20}}"#,
    ] {
        let input = format!(r#"{{"linter":{{"rules":{{"complexity":{{{entries}}}}}}}}}"#);
        let result = migrate_existing_config(&input).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        assert_eq!(
            value["linter"]["rules"]["complexity"]["max-lines-for-file"],
            serde_json::json!({"level": "warn", "options": {"max-lines": 30}})
        );
    }
}

#[test]
fn yaml_collisions_keep_highest_severity_across_upstream_linters() {
    for (dcl, pyramid) in [("true", "false"), ("false", "true")] {
        let yaml = format!(
            "dart_code_linter:\n  rules:\n    - no-empty-block: {dcl}\n\
             custom_lint:\n  rules:\n    - avoid_empty_blocks: {pyramid}\n"
        );
        let (value, unrecognized, count) = parse(&yaml);
        assert!(unrecognized.is_empty());
        assert_eq!(count, 2);
        assert_eq!(
            value["linter"]["rules"]["suspicious"]["no-empty-block"],
            "warn"
        );
    }
}

#[test]
fn yaml_collision_keeps_canonical_options_on_equal_severity() {
    let (value, unrecognized, count) = parse(
        "dart_code_linter:\n  rules:\n    - no-magic-number:\n        allowed: [1]\n\
         custom_lint:\n  rules:\n    - no_magic_number:\n        allowed: [2]\n",
    );
    assert!(unrecognized.is_empty());
    assert_eq!(count, 2);
    assert_eq!(
        value["linter"]["rules"]["style"]["no-magic-number"],
        serde_json::json!({"level": "warn", "options": {"allowed": [1]}})
    );
}

#[test]
fn yaml_duplicate_rule_entries_do_not_let_off_replace_warn() {
    let (value, unrecognized, count) =
        parse("dart_code_linter:\n  rules:\n    - no-empty-block\n    - no-empty-block: false\n");
    assert!(unrecognized.is_empty());
    assert_eq!(count, 2);
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["no-empty-block"],
        "warn"
    );
}

#[test]
fn upgrade_normalizes_option_keys_in_base_and_override_sections() {
    for options in [
        serde_json::json!({"max_lines": 20, "max-lines": 30}),
        serde_json::json!({"max-lines": 30, "max_lines": 20}),
    ] {
        let rule = serde_json::json!({"level": "error", "options": options});
        let section = serde_json::json!({"rules": {"complexity": {"max-lines-for-file": rule}}});
        let cross_file = serde_json::json!({
            "rules": {"correctness": {"unused-files": {"level": "warn", "options": {}}}}
        });
        let input = serde_json::json!({
            "linter": section,
            "cross_file": cross_file,
            "overrides": [{"includes": ["test/**"], "linter": section, "project": cross_file}]
        });
        let result = migrate_existing_config(&input.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        for scope in [&value, &value["overrides"][0]] {
            assert_eq!(
                scope["linter"]["rules"]["complexity"]["max-lines-for-file"],
                serde_json::json!({"level": "error", "options": {"max-lines": 30}})
            );
            assert_eq!(
                scope["cross-file"]["rules"]["correctness"]["unused-files"]["options"],
                serde_json::json!({})
            );
        }
        let again = migrate_existing_config(&result.json).unwrap();
        assert_eq!(again.json, result.json);
    }
}

#[test]
fn yaml_normalizes_option_keys_and_preserves_canonical_collision_values() {
    let result = migrate_yaml_to_config(
        "custom_lint:\n  rules:\n    - max_lines_for_file:\n        max_lines: 20\n        max-lines: 30\n",
    )
    .unwrap();
    let value: Value = serde_json::from_str(&result.json).unwrap();
    assert_eq!(
        value["linter"]["rules"]["complexity"]["max-lines-for-file"]["options"],
        serde_json::json!({"max-lines": 30})
    );
}

#[test]
fn yaml_rejects_unsupported_upstream_options_with_full_path() {
    let error = migrate_yaml_to_config(
        "dart_code_linter:\n  rules:\n    - check-unused-files:\n        ignored_paths: [generated/**]\n",
    )
    .err()
    .expect("unsupported upstream options fail migration");
    assert!(
        error.contains("cross-file.rules.correctness.unused-files.options.ignored-paths"),
        "{error}"
    );
}

#[test]
fn run_migrate_does_not_write_invalid_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("analysis_options.yaml");
    let output = dir.path().join("falcon.json");
    let yaml = "dart_code_linter:\n  rules:\n    - check-unused-files:\n        ignored_paths: [generated/**]\n";
    std::fs::write(&input, yaml).unwrap();
    std::fs::write(&output, "existing output").unwrap();
    assert_eq!(
        run_migrate(Some(input.clone()), true, Some(output.clone())),
        1
    );
    assert_eq!(std::fs::read_to_string(input).unwrap(), yaml);
    assert_eq!(std::fs::read_to_string(output).unwrap(), "existing output");
}

#[test]
fn upgrade_rejects_unsupported_options_with_override_path() {
    let input = r#"{"overrides":[{"linter":{"rules":{"complexity":{
        "max_lines_for_file":{"level":"warn","options":{"nested_items":[]}}
    }}}}]}"#;
    let error = migrate_existing_config(input)
        .err()
        .expect("unsupported options fail upgrade");
    assert!(
        error.contains(
            "overrides[0].linter.rules.complexity.max-lines-for-file.options.nested-items"
        ),
        "{error}"
    );
}

#[test]
fn upgrade_rejects_unknown_rule_ids_in_base_and_overrides() {
    for input in [
        r#"{"linter":{"rules":{"suspicious":{"unknown-rule":"warn"}}}}"#,
        r#"{"overrides":[{"linter":{"rules":{"suspicious":{"unknown-rule":"warn"}}}}]}"#,
    ] {
        let error = migrate_existing_config(input)
            .err()
            .expect("unknown falcon rules fail upgrade");
        let path = if input.contains("overrides") {
            "overrides[0].linter.rules.suspicious.unknown-rule"
        } else {
            "linter.rules.suspicious.unknown-rule"
        };
        assert!(error.contains("unknown rule key"), "{error}");
        assert!(error.contains(path), "{error}");
    }
}

#[test]
fn run_migrate_does_not_rewrite_config_with_unknown_rule_ids() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("falcon.json");
    let json = r#"{"linter":{"rules":{"suspicious":{
        "no_empty_block":"warn","unknown-rule":"error"
    }}}}"#;
    std::fs::write(&input, json).unwrap();
    assert_eq!(run_migrate(Some(input.clone()), true, None), 1);
    assert_eq!(std::fs::read_to_string(input).unwrap(), json);
}
