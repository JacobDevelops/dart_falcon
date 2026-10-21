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
        for path in ["cross_file", "overrides[0].cross_file"] {
            assert!(
                result.warnings.iter().any(|warning| {
                    warning.contains(&format!("`{path}`"))
                        && warning.contains("discarded value {\"enabled\":true}")
                }),
                "{path}: {:?}",
                result.warnings
            );
        }
        assert_migrated_config_loads(&result.json);
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
        assert_eq!(result.option_key_rewrites.len(), 2);
        assert_migrated_config_loads(&result.json);
        assert_eq!(result.migrated_count, 0);
        for prefix in ["linter", "overrides[0].linter"] {
            assert!(
                result.warnings.iter().any(|warning| {
                    warning.contains("duplicate option key")
                        && warning.contains(&format!(
                            "{prefix}.rules.complexity.max-lines-for-file.options"
                        ))
                        && warning.contains("20")
                        && warning.contains("discarded")
                }),
                "{:?}",
                result.warnings
            );
        }
        let again = migrate_existing_config(&result.json).unwrap();
        assert_eq!(again.json, result.json);
        assert!(again.option_key_rewrites.is_empty());
        assert!(again.warnings.is_empty(), "{:?}", again.warnings);
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
    assert_eq!(result.option_key_rewrites.len(), 1);
    assert_migrated_config_loads(&result.json);
    assert!(
        result.warnings.iter().any(|warning| {
            warning.contains("duplicate option key")
                && warning.contains("linter.rules.complexity.max-lines-for-file.options")
                && warning.contains("20")
                && warning.contains("discarded")
        }),
        "{:?}",
        result.warnings
    );
}

#[test]
fn yaml_drops_unusable_options_with_original_full_path_warning() {
    let result = migrate_yaml_to_config(
        "dart_code_linter:\n  rules:\n    - check-unused-files:\n        ignored_paths: [generated/**]\n",
    )
    .expect("unusable upstream options do not prevent migration");
    assert!(
        result.warnings.iter().any(|warning| {
            warning.contains("cross-file.rules.correctness.unused-files.options.ignored_paths")
                && warning.contains("discarded")
                && warning.contains("generated/**")
        }),
        "{:?}",
        result.warnings
    );
    assert!(result.option_key_rewrites.is_empty());
    let value: Value = serde_json::from_str(&result.json).unwrap();
    assert_eq!(
        value["cross-file"]["rules"]["correctness"]["unused-files"],
        serde_json::json!({"level": "warn"})
    );
}

#[test]
fn run_migrate_writes_yaml_output_despite_unusable_options() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("analysis_options.yaml");
    let output = dir.path().join("falcon.json");
    let yaml = "dart_code_linter:\n  rules:\n    - check-unused-files:\n        ignored_paths: [generated/**]\n";
    std::fs::write(&input, yaml).unwrap();
    std::fs::write(&output, "existing output").unwrap();
    assert_eq!(
        run_migrate(Some(input.clone()), true, Some(output.clone())),
        0
    );
    assert_eq!(std::fs::read_to_string(input).unwrap(), yaml);
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&output).unwrap()).unwrap();
    assert_eq!(
        value["cross-file"]["rules"]["correctness"]["unused-files"],
        serde_json::json!({"level": "warn"})
    );
    assert!(
        falcon_rules::load_config(&output)
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn upgrade_drops_unknown_option_with_exact_original_override_path() {
    let input = r#"{"overrides":[{"linter":{"rules":{"complexity":{
        "max_lines_for_file":{"level":"warn","options":{"ignored_paths":["generated/**"]}}
    }}}}]}"#;
    let result =
        migrate_existing_config(input).expect("unsupported options do not prevent upgrade");
    assert!(
        result.warnings.iter().any(|warning| {
            warning.contains(
                "`overrides[0].linter.rules.complexity.max-lines-for-file.options.ignored_paths`",
            ) && warning.contains("discarded value [\"generated/**\"]")
        }),
        "{:?}",
        result.warnings
    );
    assert!(result.option_key_rewrites.is_empty());
    let value: Value = serde_json::from_str(&result.json).unwrap();
    assert_eq!(
        value["overrides"][0]["linter"]["rules"]["complexity"]["max-lines-for-file"]["options"],
        serde_json::json!({})
    );
    assert_migrated_config_loads(&result.json);
}

#[test]
fn upgrade_drops_unknown_rule_ids_in_base_and_overrides() {
    for input in [
        r#"{"linter":{"rules":{"suspicious":{"unknown_rule":"warn"}}}}"#,
        r#"{"overrides":[{"linter":{"rules":{"suspicious":{"unknown_rule":"warn"}}}}]}"#,
    ] {
        let result = migrate_existing_config(input).expect("unknown rules do not prevent upgrade");
        let (path, pointer) = if input.contains("overrides") {
            (
                "overrides[0].linter.rules.suspicious.unknown_rule",
                "/overrides/0/linter/rules/suspicious/unknown_rule",
            )
        } else {
            (
                "linter.rules.suspicious.unknown_rule",
                "/linter/rules/suspicious/unknown_rule",
            )
        };
        assert_eq!(result.unrecognized, ["unknown_rule"]);
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(path) && warning.contains("discarded value \"warn\"")
            }),
            "{:?}",
            result.warnings
        );
        let mut expected: Value = serde_json::from_str(input).unwrap();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        expected
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert_eq!(
            serde_json::from_str::<Value>(&result.json).unwrap(),
            expected
        );
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn run_migrate_rewrites_known_ids_and_drops_unknown_ids() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("falcon.json");
    let json = r#"{"linter":{"rules":{"suspicious":{
        "no_empty_block":"warn","unknown-rule":"error"
    }}}}"#;
    std::fs::write(&input, json).unwrap();
    assert_eq!(run_migrate(Some(input.clone()), true, None), 0);
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&input).unwrap()).unwrap();
    assert_eq!(
        value["linter"]["rules"]["suspicious"],
        serde_json::json!({"no-empty-block": "warn"})
    );
    falcon_rules::load_config(&input).expect("written config passes canonical loading");
}

#[test]
fn yaml_drops_unsupported_options_with_full_paths_and_values() {
    let result = migrate_yaml_to_config(
        "dart_code_linter:\n  rules:\n    - avoid-nested-conditional-expressions:\n        acceptable-level: 2\n    - prefer-correct-identifier-length:\n        max-identifier-length: 30\n    - member-ordering:\n        alphabetize: true\n",
    )
    .expect("unsupported upstream keys do not prevent migration");
    let value: Value = serde_json::from_str(&result.json).unwrap();
    let rules = &value["linter"]["rules"];
    for (name, key, discarded, expected) in [
        (
            "avoid-nested-conditional-expressions",
            "acceptable-level",
            "2",
            serde_json::json!({"level": "warn"}),
        ),
        (
            "prefer-correct-identifier-length",
            "max-identifier-length",
            "30",
            serde_json::json!({"level": "warn", "options": {}}),
        ),
        (
            "member-ordering",
            "alphabetize",
            "true",
            serde_json::json!({"level": "warn", "options": {}}),
        ),
    ] {
        let meta = falcon_rules::meta::meta_for(name).unwrap();
        assert_eq!(rules[meta.group.as_str()][name], expected);
        let path = format!("linter.rules.{}.{name}.options.{key}", meta.group.as_str());
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(&format!("`{path}`"))
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{name}: {:?}",
            result.warnings
        );
    }
    assert_eq!(result.migrated_count, 3);
    assert!(result.unrecognized.is_empty());
    assert_migrated_config_loads(&result.json);
}

#[test]
fn malformed_option_values_are_dropped_and_written_with_warnings() {
    for (options, discarded) in [
        (serde_json::json!({"max_lines": "many"}), "\"many\""),
        (serde_json::json!({"max-lines": null}), "null"),
        (serde_json::json!(["not-an-object"]), "[\"not-an-object\"]"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("falcon.json");
        let original = serde_json::json!({
            "linter": {"rules": {"complexity": {
                "max_lines_for_file": {"level": "warn", "options": options}
            }}}
        });
        let result = migrate_existing_config(&original.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        let expected = if options.is_array() {
            serde_json::json!({"level": "warn"})
        } else {
            serde_json::json!({"level": "warn", "options": {}})
        };
        assert_eq!(
            value["linter"]["rules"]["complexity"]["max-lines-for-file"],
            expected
        );
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains("linter.rules.complexity.max-lines-for-file.options")
                    && warning.contains("invalid rule options")
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{:?}",
            result.warnings
        );
        std::fs::write(&input, original.to_string()).unwrap();
        assert_eq!(run_migrate(Some(input.clone()), true, None), 0);
        assert_eq!(std::fs::read_to_string(&input).unwrap(), result.json);
        falcon_rules::load_config(&input)
            .expect("written malformed migration passes strict loading");
    }
}

#[test]
fn malformed_rule_structure_is_dropped_without_losing_valid_siblings() {
    let input = r#"{"linter":{"rules":{
        "complexity":{"max_lines_for_file":{"level":"invalid","options":{"max_lines":20}}},
        "suspicious":{"no_empty_block":"warn"}
    }}}"#;
    let result = migrate_existing_config(input).unwrap();
    let value: Value = serde_json::from_str(&result.json).unwrap();
    assert!(
        value["linter"]["rules"]["complexity"]
            .get("max-lines-for-file")
            .is_none()
    );
    assert_eq!(
        value["linter"]["rules"]["suspicious"]["no-empty-block"],
        "warn"
    );
    assert!(
        result.warnings.iter().any(|warning| {
            warning.contains("linter.rules.complexity.max-lines-for-file")
                && warning.contains("invalid")
                && warning.contains("discarded value")
        }),
        "{:?}",
        result.warnings
    );
    assert_eq!(result.migrated_count, 2);
    assert_eq!(result.option_key_rewrites.len(), 1);
    assert_migrated_config_loads(&result.json);
}

#[test]
fn yaml_drops_malformed_values_with_exact_option_path() {
    let result = migrate_yaml_to_config(
        "custom_lint:\n  rules:\n    - max_lines_for_file:\n        max_lines: many\n",
    )
    .expect("malformed option values do not prevent YAML conversion");
    let value: Value = serde_json::from_str(&result.json).unwrap();
    assert_eq!(
        value["linter"]["rules"]["complexity"]["max-lines-for-file"]["options"],
        serde_json::json!({})
    );
    assert!(
        result.warnings.iter().any(|warning| {
            warning.contains("`linter.rules.complexity.max-lines-for-file.options.max-lines`")
                && warning.contains("invalid rule options")
                && warning.contains("discarded value \"many\"")
        }),
        "{:?}",
        result.warnings
    );
    assert_eq!(result.option_key_rewrites.len(), 1);
    assert_migrated_config_loads(&result.json);
}

fn assert_migrated_config_loads(json: &str) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("falcon.json");
    std::fs::write(&path, json).unwrap();
    falcon_rules::load_config(&path).expect("migration output passes actual canonical loading");
}

#[test]
fn upgrade_drops_bad_list_entries_and_preserves_original_warning_indices() {
    let options = serde_json::json!({
        "allowed": [1, "bad", false, 2, null],
        "ignored_paths": ["generated/**"]
    });
    let section = serde_json::json!({
        "rules": {"style": {"no-magic-number": {"level": "warn", "options": options}}}
    });
    let input = serde_json::json!({
        "linter": section,
        "overrides": [{"includes": ["test/**"], "linter": section}]
    });
    let result = migrate_existing_config(&input.to_string()).unwrap();
    let value: Value = serde_json::from_str(&result.json).unwrap();
    for (scope, prefix) in [
        (&value, "linter"),
        (&value["overrides"][0], "overrides[0].linter"),
    ] {
        assert_eq!(
            scope["linter"]["rules"]["style"]["no-magic-number"]["options"],
            serde_json::json!({"allowed": [1, 2]})
        );
        for (suffix, discarded) in [
            ("allowed[1]", "\"bad\""),
            ("allowed[2]", "false"),
            ("allowed[4]", "null"),
            ("ignored_paths", "[\"generated/**\"]"),
        ] {
            let path = format!("{prefix}.rules.style.no-magic-number.options.{suffix}");
            assert!(
                result.warnings.iter().any(|warning| {
                    warning.contains(&format!("`{path}`"))
                        && warning.contains(&format!("discarded value {discarded}"))
                }),
                "{path}: {:?}",
                result.warnings
            );
        }
    }
    assert!(result.option_key_rewrites.is_empty());
    assert_migrated_config_loads(&result.json);
}

#[test]
fn upgrade_repairs_malformed_structural_fields_with_full_discard_warnings() {
    for (key, invalid, expected_path, discarded) in [
        (
            "enabled",
            serde_json::json!("yes"),
            "linter.enabled",
            "\"yes\"",
        ),
        (
            "domains",
            serde_json::json!({"not-a-domain": "all"}),
            "linter.domains.not-a-domain",
            "\"all\"",
        ),
        (
            "domains",
            serde_json::json!({"flutter": "invalid"}),
            "linter.domains.flutter",
            "\"invalid\"",
        ),
    ] {
        let mut input = serde_json::json!({
            "linter": {"rules": {"suspicious": {"no_empty_block": "warn"}}}
        });
        input["linter"][key] = invalid;
        let result = migrate_existing_config(&input.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        assert_eq!(
            value["linter"]["rules"]["suspicious"]["no-empty-block"],
            "warn"
        );
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(&format!("`{expected_path}`"))
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{expected_path}: {:?}",
            result.warnings
        );
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn upgrade_repairs_root_and_override_shapes_without_suppressing_discarded_values() {
    for (input, path, discarded) in [
        (serde_json::json!({"max-errors": 0}), "max-errors", "0"),
        (serde_json::json!({"unknown_key": 7}), "unknown_key", "7"),
        (
            serde_json::json!({"files": {"includes": ["lib/**", 42, false]}}),
            "files.includes[1]",
            "42",
        ),
        (
            serde_json::json!({"overrides": [42, false, {"includes": ["test/**"]}]}),
            "overrides[0]",
            "42",
        ),
    ] {
        let result = migrate_existing_config(&input.to_string()).unwrap();
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(&format!("`{path}`"))
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{path}: {:?}",
            result.warnings
        );
        let value: Value = serde_json::from_str(&result.json).unwrap();
        if input.get("files").is_some() {
            assert_eq!(value["files"]["includes"], serde_json::json!(["lib/**"]));
            assert!(result.warnings.iter().any(|warning| {
                warning.contains("`files.includes[2]`") && warning.contains("discarded value false")
            }));
        }
        if input.get("overrides").is_some() {
            assert_eq!(
                value["overrides"],
                serde_json::json!([{"includes": ["test/**"]}])
            );
            assert!(result.warnings.iter().any(|warning| {
                warning.contains("`overrides[1]`") && warning.contains("discarded value false")
            }));
        }
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn yaml_discards_malformed_payloads_and_list_entries_before_strict_loading() {
    for (payload, discarded) in [("[bad]", "[\"bad\"]"), ("many", "\"many\""), ("42", "42")] {
        let yaml = format!("custom_lint:\n  rules:\n    - max_lines_for_file: {payload}\n");
        let result = migrate_yaml_to_config(&yaml).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        assert_eq!(
            value["linter"]["rules"]["complexity"]["max-lines-for-file"],
            serde_json::json!({"level": "warn"})
        );
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains("`linter.rules.complexity.max-lines-for-file.options`")
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{:?}",
            result.warnings
        );
        assert_migrated_config_loads(&result.json);
    }
    let result = migrate_yaml_to_config(
        "dart_code_linter:\n  rules:\n    - no-magic-number:\n        allowed: [1, bad, false, 2]\n        ignored_paths: [generated/**]\n",
    ).unwrap();
    let value: Value = serde_json::from_str(&result.json).unwrap();
    assert_eq!(
        value["linter"]["rules"]["style"]["no-magic-number"]["options"],
        serde_json::json!({"allowed": [1, 2]})
    );
    for (suffix, discarded) in [
        ("allowed[1]", "\"bad\""),
        ("allowed[2]", "false"),
        ("ignored_paths", "[\"generated/**\"]"),
    ] {
        let path = format!("linter.rules.style.no-magic-number.options.{suffix}");
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(&format!("`{path}`"))
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{path}: {:?}",
            result.warnings
        );
    }
    assert_migrated_config_loads(&result.json);
}

#[test]
fn upgrade_reports_discarded_root_payloads_and_keeps_outputs_loadable() {
    for input in ["null", "42", "[]", r#""not-an-object""#] {
        let result = migrate_existing_config(input).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result.json).unwrap(),
            serde_json::json!({})
        );
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(&format!("discarded value {input}")) && warning.contains("`$`")
            }),
            "{:?}",
            result.warnings
        );
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn yaml_unknown_rules_and_malformed_entries_report_original_paths_and_values() {
    let result = migrate_yaml_to_config(
        "custom_lint:\n  rules:\n    - unknown_rule:\n        ignored_paths: [generated/**]\n    - 42\nlinter:\n  rules:\n    unknown_official_rule: false\n",
    ).unwrap();
    assert_eq!(
        result.unrecognized,
        ["unknown_rule", "unknown_official_rule"]
    );
    for (path, discarded) in [
        (
            "custom_lint.rules[0].unknown_rule",
            "{\"ignored_paths\":[\"generated/**\"]}",
        ),
        ("custom_lint.rules[1]", "42"),
        ("linter.rules.unknown_official_rule", "false"),
    ] {
        assert!(
            result.warnings.iter().any(|warning| {
                warning.contains(&format!("`{path}`"))
                    && warning.contains(&format!("discarded value {discarded}"))
            }),
            "{path}: {:?}",
            result.warnings
        );
    }
    assert_eq!(result.migrated_count, 0);
    assert_migrated_config_loads(&result.json);
}

#[test]
fn upgrade_rule_collisions_report_exact_base_and_override_paths_with_json_values() {
    for (legacy, canonical, discarded) in [
        (
            serde_json::json!("warn"),
            serde_json::json!("error"),
            "\"warn\"",
        ),
        (
            serde_json::json!({"level": "warn", "options": {"max-lines": 20}}),
            serde_json::json!({"level": "error", "options": {"max-lines": 30}}),
            "{\"level\":\"warn\",\"options\":{\"max-lines\":20}}",
        ),
    ] {
        let section = serde_json::json!({
            "rules": {"complexity": {
                "max_lines_for_file": legacy,
                "max-lines-for-file": canonical,
            }}
        });
        let input = serde_json::json!({
            "linter": section,
            "overrides": [{"includes": ["test/**"], "linter": section}]
        });
        let result = migrate_existing_config(&input.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        for (scope, prefix) in [
            (&value, "linter"),
            (&value["overrides"][0], "overrides[0].linter"),
        ] {
            assert_eq!(
                scope["linter"]["rules"]["complexity"]["max-lines-for-file"],
                canonical
            );
            let diagnostic = format!(
                "duplicate rule key `{prefix}.rules.complexity.max-lines-for-file`: \
                 discarded configuration {discarded};"
            );
            assert!(
                result
                    .warnings
                    .iter()
                    .any(|warning| warning.starts_with(&diagnostic)),
                "{diagnostic}: {:?}",
                result.warnings
            );
        }
        assert_eq!(result.migrated_count, 2);
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn yaml_rule_collisions_report_exact_paths_and_discarded_json_configurations() {
    for (yaml, path, discarded, expected) in [
        (
            "dart_code_linter:\n  rules:\n    - no-empty-block\n    - no-empty-block: false\n",
            "linter.rules.suspicious.no-empty-block",
            "\"off\"",
            serde_json::json!("warn"),
        ),
        (
            "dart_code_linter:\n  rules:\n    - no-magic-number:\n        allowed: [1]\ncustom_lint:\n  rules:\n    - no_magic_number:\n        allowed: [2]\n",
            "linter.rules.style.no-magic-number",
            "{\"level\":\"warn\",\"options\":{\"allowed\":[2]}}",
            serde_json::json!({"level": "warn", "options": {"allowed": [1]}}),
        ),
    ] {
        let result = migrate_yaml_to_config(yaml).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        let pointer = format!("/{}", path.replace('.', "/"));
        assert_eq!(value.pointer(&pointer).unwrap(), &expected);
        let diagnostic =
            format!("duplicate rule key `{path}`: discarded configuration {discarded};");
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.starts_with(&diagnostic)),
            "{diagnostic}: {:?}",
            result.warnings
        );
        assert_eq!(result.migrated_count, 2);
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn upgrade_discards_invalid_rule_levels_in_base_and_overrides_without_losing_valid_siblings() {
    for (invalid, discarded) in [
        (serde_json::json!("invalid"), "\"invalid\""),
        (serde_json::json!({"options": {}}), "{\"options\":{}}"),
        (serde_json::json!(true), "true"),
        (serde_json::json!({"level": false}), "false"),
        (serde_json::json!({"level": "invalid"}), "\"invalid\""),
    ] {
        let section = serde_json::json!({
            "rules": {
                "complexity": {"max_lines_for_file": invalid},
                "suspicious": {"no_empty_block": "warn"}
            }
        });
        let input = serde_json::json!({
            "linter": section,
            "overrides": [{"includes": ["test/**"], "linter": section}]
        });
        let result = migrate_existing_config(&input.to_string()).unwrap();
        let value: Value = serde_json::from_str(&result.json).unwrap();
        for (scope, prefix) in [
            (&value, "linter"),
            (&value["overrides"][0], "overrides[0].linter"),
        ] {
            assert!(
                scope["linter"]["rules"]["complexity"]
                    .get("max-lines-for-file")
                    .is_none(),
                "{invalid}: {scope}"
            );
            assert_eq!(
                scope["linter"]["rules"]["suspicious"]["no-empty-block"],
                "warn"
            );
            let path = format!("{prefix}.rules.complexity.max-lines-for-file");
            assert!(
                result.warnings.iter().any(|warning| {
                    warning.contains(&path)
                        && warning.contains(&format!("discarded value {discarded}"))
                }),
                "{path}, {invalid}: {:?}",
                result.warnings
            );
        }
        assert_eq!(result.migrated_count, 4);
        assert_migrated_config_loads(&result.json);
    }
}

#[test]
fn optionless_rule_migration_reports_each_discarded_key_and_removes_the_payload() {
    let yaml = migrate_yaml_to_config(
        "dart_code_linter:\n  rules:\n    - check-unused-files:\n        ignored_paths: [generated/**]\n        unsupported: 42\n",
    ).unwrap();
    let upgrade = migrate_existing_config(
        r#"{"cross-file":{"rules":{"correctness":{"unused-files":{
            "level":"warn","options":{"ignored_paths":["generated/**"],"unsupported":42}
        }}}}}"#,
    )
    .unwrap();
    for result in [yaml, upgrade] {
        let value: Value = serde_json::from_str(&result.json).unwrap();
        assert_eq!(
            value["cross-file"]["rules"]["correctness"]["unused-files"],
            serde_json::json!({"level": "warn"})
        );
        for (key, discarded) in [
            ("ignored_paths", "[\"generated/**\"]"),
            ("unsupported", "42"),
        ] {
            let path = format!("cross-file.rules.correctness.unused-files.options.{key}");
            assert!(
                result.warnings.iter().any(|warning| {
                    warning.contains(&format!("`{path}`"))
                        && warning.contains(&format!("discarded value {discarded}"))
                }),
                "{path}: {:?}",
                result.warnings
            );
        }
        assert!(result.option_key_rewrites.is_empty());
        assert_migrated_config_loads(&result.json);
    }
}
