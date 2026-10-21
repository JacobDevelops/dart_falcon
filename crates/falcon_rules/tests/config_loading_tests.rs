use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use falcon_rules::{load_config, load_or_default};
use serde_json::json;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "falcon-canonical-loader-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn discovery_without_config_returns_defaults() {
    let directory = directory();
    let loaded = load_or_default(&directory).unwrap();
    assert!(loaded.config.linter.enabled);
    assert_eq!(loaded.config.max_errors, None);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn discovery_uses_the_canonical_loader_and_propagates_errors() {
    let directory = directory();
    let path = directory.join("falcon.json");
    fs::write(
        &path,
        json!({
            "linter": { "rules": {
                "recommended": true,
                "style": { "no_magic_number": "warn" }
            } },
            "max_errors": 50
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_or_default(&directory).unwrap();
    assert_eq!(loaded.config.max_errors, Some(50));
    assert_eq!(loaded.config.linter.rules.recommended, Some(true));
    assert!(loaded.config.linter.rules.groups["style"].contains_key("no-magic-number"));
    assert_eq!(loaded.warnings.len(), 2);

    for (value, expected) in [
        (json!({ "max-errors": "100" }), "invalid type"),
        (
            json!({ "rules": { "avoid-dynamic": { "enabled": false } } }),
            "legacy flat schema",
        ),
        (
            json!({ "overrides": [{ "include": ["test/**"] }] }),
            "overrides[0].include",
        ),
        (
            json!({ "linter": { "domains": { "fluter": "recommended" } } }),
            "linter.domains.fluter",
        ),
    ] {
        fs::write(&path, value.to_string()).unwrap();
        let error = load_or_default(&directory).unwrap_err();
        assert!(error.0.contains(expected), "{error}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn known_rules_in_nonexistent_slots_are_errors_with_full_paths() {
    let directory = directory();
    let path = directory.join("falcon.json");
    for (value, expected) in [
        (
            json!({ "linter": { "rules": { "complexity": { "no-magic-number": "warn" } } } }),
            "linter.rules.complexity.no-magic-number",
        ),
        (
            json!({ "linter": { "rules": { "correctness": { "unused-files": "warn" } } } }),
            "linter.rules.correctness.unused-files",
        ),
        (
            json!({ "cross-file": { "rules": { "style": { "no_magic_number": "warn" } } } }),
            "cross-file.rules.style.no_magic_number",
        ),
        (
            json!({ "overrides": [{ "linter": { "rules": { "correctness": { "unused-files": "off" } } } }] }),
            "overrides[0].linter.rules.correctness.unused-files",
        ),
        (
            json!({ "overrides": [{ "cross-file": { "rules": { "style": { "no-magic-number": "off" } } } }] }),
            "overrides[0].cross-file.rules.style.no-magic-number",
        ),
        (
            json!({ "overrides": [{ "linter": { "rules": { "recommended": false } } }] }),
            "overrides[0].linter.rules.recommended",
        ),
        (
            json!({ "overrides": [{ "cross-file": { "rules": { "recommended": false } } }] }),
            "overrides[0].cross-file.rules.recommended",
        ),
    ] {
        fs::write(&path, value.to_string()).unwrap();
        let explicit = load_config(&path).unwrap_err();
        let discovered = load_or_default(&directory).unwrap_err();
        assert!(explicit.0.contains(expected), "{explicit}");
        assert_eq!(explicit.0, discovered.0);
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn structural_validator_groups_and_domains_match_registry_metadata() {
    use falcon_rules::meta::{Domain, RuleGroup};

    assert_eq!(
        falcon_config::RULE_GROUPS,
        RuleGroup::ALL
            .iter()
            .map(|group| group.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        falcon_config::DOMAINS,
        Domain::ALL
            .iter()
            .map(|domain| domain.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn override_schema_rejects_presets_but_base_sections_allow_them() {
    let schema = falcon_rules::schema::config_schema();
    for section in ["linter", "cross-file"] {
        assert!(
            schema["properties"][section]["properties"]["rules"]["properties"]
                .get("recommended")
                .is_some()
        );
    }
    for definition in ["overrideLinter", "overrideCrossFile"] {
        let rules = &schema["definitions"][definition]["properties"]["rules"];
        assert!(rules["properties"].get("recommended").is_none());
        assert_eq!(rules["additionalProperties"], false);
    }
}

#[test]
fn typed_options_reject_invalid_values_with_original_full_paths() {
    use falcon_config::ConfigPathSegment::{Index, Key};
    let directory = directory();
    let path = directory.join("falcon.json");
    for (section, group, rule, options, field, segments) in [
        (
            "linter",
            "complexity",
            "max-parameters-for-function",
            json!({ "unknown_key": 1 }),
            "unknown_key",
            vec![Key("unknown_key".into())],
        ),
        (
            "linter",
            "complexity",
            "max-parameters-for-function",
            json!({ "max_parameters": "2" }),
            "max_parameters",
            vec![Key("max_parameters".into())],
        ),
        (
            "linter",
            "complexity",
            "max-parameters-for-function",
            json!({ "max-parameters": -1 }),
            "max-parameters",
            vec![Key("max-parameters".into())],
        ),
        (
            "linter",
            "style",
            "prefer-correct-handler-name",
            json!({ "name_pattern": "[" }),
            "name_pattern",
            vec![Key("name_pattern".into())],
        ),
        (
            "linter",
            "style",
            "use-design-system-item",
            json!({ "items": [{ "class_name": 42 }] }),
            "items[0].class_name",
            vec![Key("items".into()), Index(0), Key("class_name".into())],
        ),
        (
            "linter",
            "style",
            "use-design-system-item",
            json!({ "items": [{ "class-name": "Container", "unknown_key": true }] }),
            "items[0].unknown_key",
            vec![Key("items".into()), Index(0), Key("unknown_key".into())],
        ),
        (
            "linter",
            "style",
            "class-members-ordering",
            json!({ "order": ["not-a-member-category"] }),
            "order[0]",
            vec![Key("order".into()), Index(0)],
        ),
        (
            "cross-file",
            "correctness",
            "banned-imports",
            json!({ "rules": "not-a-list" }),
            "rules",
            vec![Key("rules".into())],
        ),
        (
            "cross-file",
            "correctness",
            "banned-imports",
            json!({ "rules": [{
                "files": ["lib/**"], "deny": ["package:flutter/**"],
                "message": "No Flutter", "unknown_key": true
            }] }),
            "rules[0].unknown_key",
            vec![Key("rules".into()), Index(0), Key("unknown_key".into())],
        ),
        (
            "cross-file",
            "correctness",
            "banned-imports",
            json!({ "rules": [{
                "files": [], "deny": ["package:flutter/**"], "message": "No Flutter"
            }] }),
            "rules[0].files",
            vec![Key("rules".into()), Index(0), Key("files".into())],
        ),
    ] {
        for in_override in [false, true] {
            let mut value = json!({});
            value[section] = json!({ "rules": {
                (group): { (rule): { "level": "warn", "options": options.clone() } }
            } });
            let prefix = if in_override { "overrides[0]." } else { "" };
            if in_override {
                value["includes"] = json!(["lib/**"]);
                value = json!({ "overrides": [value] });
            }
            fs::write(&path, value.to_string()).unwrap();
            let error = load_config(&path).unwrap_err();
            let expected = format!("{prefix}{section}.rules.{group}.{rule}.options.{field}");
            assert!(error.0.contains(&expected), "expected {expected}: {error}");
            let mut expected_segments = Vec::with_capacity(7 + segments.len());
            if in_override {
                expected_segments.extend([Key("overrides".into()), Index(0)]);
            }
            expected_segments
                .extend([section, "rules", group, rule, "options"].map(|key| Key(key.into())));
            expected_segments.extend_from_slice(&segments);
            assert_eq!(error.1, expected_segments, "{error}");
        }
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn deprecated_option_keys_warn_with_paths_and_canonical_keys_win() {
    let directory = directory();
    let path = directory.join("falcon.json");
    fs::write(
        &path,
        json!({
            "linter": { "rules": {
                "complexity": { "max-parameters-for-function": {
                    "level": "warn", "options": { "max_parameters": 2 }
                } },
                "style": { "prefer-correct-handler-name": {
                    "level": "warn",
                    "options": { "name_pattern": "[", "name-pattern": "^handle" }
                } }
            } },
            "overrides": [{
                "includes": ["test/**"],
                "linter": { "rules": { "complexity": {
                    "max-parameters-for-function": {
                        "level": "warn",
                        "options": { "max_parameters": -1, "max-parameters": 3 }
                    }
                } } }
            }]
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.warnings.len(), 5);
    for expected in [
        "linter.rules.complexity.max-parameters-for-function.options.max_parameters",
        "linter.rules.style.prefer-correct-handler-name.options.name_pattern",
        "overrides[0].linter.rules.complexity.max-parameters-for-function.options.max_parameters",
    ] {
        assert!(
            loaded.warnings.iter().any(|warning| {
                warning.contains("deprecated option key") && warning.contains(expected)
            }),
            "missing deprecation warning for {expected}: {:?}",
            loaded.warnings
        );
    }
    for expected in [
        "duplicate option key `linter.rules.style.prefer-correct-handler-name.options.name-pattern`: discarded \"[\"",
        "duplicate option key `overrides[0].linter.rules.complexity.max-parameters-for-function.options.max-parameters`: discarded -1",
    ] {
        assert!(loaded.warnings.iter().any(|warning| warning == expected));
    }
    assert_eq!(
        loaded
            .config
            .rule_options_data_for(
                &directory.join("lib/a.dart").to_string_lossy(),
                "complexity",
                "max-parameters-for-function",
            )
            .map(falcon_config::RuleOptions::value),
        Some(&json!({ "max-parameters": 2 }))
    );
    assert_eq!(
        loaded
            .config
            .rule_options_data_for(
                &directory.join("lib/a.dart").to_string_lossy(),
                "style",
                "prefer-correct-handler-name",
            )
            .map(falcon_config::RuleOptions::value),
        Some(&json!({ "name-pattern": "^handle" }))
    );
    assert_eq!(
        loaded
            .config
            .rule_options_data_for(
                &directory.join("test/a.dart").to_string_lossy(),
                "complexity",
                "max-parameters-for-function"
            )
            .map(falcon_config::RuleOptions::value),
        Some(&json!({ "max-parameters": 3 }))
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unknown_snake_case_option_keys_keep_their_spelling() {
    let mut options = json!({ "not_a_setting": 4 });
    assert!(
        falcon_rules::canonicalize_option_keys(
            &mut options,
            "max-parameters-for-function",
            "linter.rules.complexity.max-parameters-for-function.options",
        )
        .is_empty()
    );
    assert_eq!(options, json!({ "not_a_setting": 4 }));

    let directory = directory();
    let path = directory.join("falcon.json");
    fs::write(
        &path,
        json!({
            "linter": { "rules": { "complexity": {
                "max-parameters-for-function": { "level": "warn", "options": options }
            } } }
        })
        .to_string(),
    )
    .unwrap();
    let error = load_config(&path).unwrap_err();
    assert!(
        error.0.contains(
            "`linter.rules.complexity.max-parameters-for-function.options.not_a_setting`"
        ),
        "{error}"
    );
    assert!(error.0.contains("unknown field `not_a_setting`"), "{error}");
    assert!(!error.0.contains("not-a-setting"), "{error}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn typed_options_follow_matching_overrides_and_preserve_level_only_changes() {
    use falcon_analyze::{AnalyzeContext, ParsedFile, RuleAnalysis};
    use falcon_rules::lint::complexity::max_parameters_for_function::MaxParametersForFunction;
    use std::sync::Arc;

    let directory = directory();
    let path = directory.join("falcon.json");
    fs::write(
        &path,
        json!({
            "linter": { "rules": {
                "complexity": { "max-parameters-for-function": {
                    "level": "warn", "options": { "max-parameters": 1 }
                } },
                "style": { "prefer-correct-handler-name": {
                    "level": "warn",
                    "options": { "name-pattern": "^handle", "ignore-public": true }
                } }
            } },
            "overrides": [
                {
                    "includes": ["test/**"],
                    "linter": { "rules": {
                        "complexity": { "max-parameters-for-function": {
                            "level": "warn", "options": { "max-parameters": 3 }
                        } },
                        "style": { "prefer-correct-handler-name": {
                            "level": "warn", "options": { "name-pattern": "^on" }
                        } }
                    } }
                },
                {
                    "includes": ["test/special/**"],
                    "linter": { "rules": { "complexity": {
                        "max-parameters-for-function": {
                            "level": "warn", "options": { "max-parameters": 2 }
                        }
                    } } }
                },
                {
                    "includes": ["test/**"],
                    "linter": { "rules": {
                        "complexity": { "max-parameters-for-function": "error" },
                        "style": { "prefer-correct-handler-name": { "level": "error" } }
                    } }
                }
            ]
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_config(&path).unwrap();
    assert!(loaded.warnings.is_empty());
    let source = "void f(int a, int b, int c) {}";
    let falcon_dart_parser::ParseResult { tree, .. } = falcon_dart_parser::parser::parse(source);
    let parsed = Arc::new(ParsedFile {
        source: source.into(),
        tree,
    });
    let rule = MaxParametersForFunction;
    for (file, maximum, reports) in [
        ("lib/a.dart", 1, 1),
        ("test/a.dart", 3, 0),
        ("test/special/a.dart", 2, 1),
    ] {
        let file_path = directory.join(file);
        assert_eq!(
            loaded
                .config
                .rule_options_data_for(
                    &file_path.to_string_lossy(),
                    "complexity",
                    "max-parameters-for-function"
                )
                .map(falcon_config::RuleOptions::value),
            Some(&json!({ "max-parameters": maximum }))
        );
        let ctx = AnalyzeContext::new(&file_path, &loaded.config, &parsed);
        assert_eq!(
            rule.analyze(MaxParametersForFunction::NAME, parsed.program(), &ctx)
                .len(),
            reports,
            "{file}"
        );
    }
    assert_eq!(
        loaded
            .config
            .rule_options_data_for(
                &directory.join("test/a.dart").to_string_lossy(),
                "style",
                "prefer-correct-handler-name"
            )
            .map(falcon_config::RuleOptions::value),
        Some(&json!({ "name-pattern": "^on" })),
        "override options replace the entire object, not just supplied fields"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn null_options_remain_absent_and_do_not_clear_previous_options() {
    let directory = directory();
    let path = directory.join("falcon.json");
    fs::write(
        &path,
        json!({
            "linter": { "rules": { "complexity": {
                "max-parameters-for-function": {
                    "level": "warn", "options": { "max-parameters": 2 }
                }
            } } },
            "cross-file": { "rules": { "correctness": {
                "banned-imports": { "level": "warn", "options": null }
            } } },
            "overrides": [{
                "includes": ["test/**"],
                "linter": { "rules": { "complexity": {
                    "max-parameters-for-function": { "level": "error", "options": null }
                } } }
            }]
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_config(&path).unwrap();
    assert!(loaded.warnings.is_empty());
    assert_eq!(
        loaded
            .config
            .rule_options_data_for(
                &directory.join("test/a.dart").to_string_lossy(),
                "complexity",
                "max-parameters-for-function"
            )
            .map(falcon_config::RuleOptions::value),
        Some(&json!({ "max-parameters": 2 }))
    );
    assert!(
        loaded
            .config
            .cross_file_options_data_for(
                &directory.join("lib/a.dart").to_string_lossy(),
                "correctness",
                "banned-imports"
            )
            .is_none()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cross_file_typed_options_replace_boundaries_in_override_order() {
    let directory = directory();
    let path = directory.join("falcon.json");
    let base = json!({ "rules": [{
        "files": ["lib/**"], "deny": ["package:flutter/**"], "message": "Base boundary"
    }] });
    let replacement = json!({ "rules": [{
        "files": ["test/**"], "deny": ["dart:io"], "message": "Test boundary"
    }] });
    fs::write(
        &path,
        json!({
            "cross-file": { "rules": { "correctness": {
                "banned-imports": { "level": "warn", "options": base }
            } } },
            "overrides": [
                {
                    "includes": ["test/**"],
                    "cross-file": { "rules": { "correctness": {
                        "banned-imports": { "level": "warn", "options": replacement }
                    } } }
                },
                {
                    "includes": ["test/**"],
                    "cross-file": { "rules": { "correctness": { "banned-imports": "error" } } }
                },
                {
                    "includes": ["test/empty/**"],
                    "cross-file": { "rules": { "correctness": {
                        "banned-imports": { "level": "warn", "options": { "rules": [] } }
                    } } }
                }
            ]
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_config(&path).unwrap();
    assert!(loaded.warnings.is_empty());
    for (file, expected) in [
        ("lib/a.dart", &base),
        ("test/a.dart", &replacement),
        ("test/empty/a.dart", &json!({ "rules": [] })),
    ] {
        assert_eq!(
            loaded
                .config
                .cross_file_options_data_for(
                    &directory.join(file).to_string_lossy(),
                    "correctness",
                    "banned-imports"
                )
                .map(|options| options.value()),
            Some(expected),
            "{file}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn nested_option_aliases_normalize_inside_arrays_with_canonical_precedence() {
    let directory = directory();
    let path = directory.join("falcon.json");
    fs::write(
        &path,
        json!({
            "linter": { "rules": { "style": { "use-design-system-item": {
                "level": "warn",
                "options": { "items": [{
                    "class_name": 42,
                    "class-name": "Container",
                    "use_instead": "AppContainer"
                }] }
            } } } }
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.warnings.len(), 3);
    for field in ["class_name", "use_instead"] {
        let expected =
            format!("linter.rules.style.use-design-system-item.options.items[0].{field}");
        assert!(
            loaded.warnings.iter().any(|warning| {
                warning.contains("deprecated option key") && warning.contains(&expected)
            }),
            "missing deprecation warning for {expected}: {:?}",
            loaded.warnings
        );
    }
    assert!(loaded.warnings.iter().any(|warning| warning ==
        "duplicate option key `linter.rules.style.use-design-system-item.options.items[0].class-name`: discarded 42"
    ));
    assert_eq!(
        loaded
            .config
            .rule_options_data_for(
                &directory.join("lib/a.dart").to_string_lossy(),
                "style",
                "use-design-system-item",
            )
            .map(falcon_config::RuleOptions::value),
        Some(&json!({ "items": [{
            "class-name": "Container", "use-instead": "AppContainer"
        }] }))
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn banned_imports_rejects_previously_ignored_invalid_boundaries() {
    let directory = directory();
    let path = directory.join("falcon.json");
    for (entry, suffix) in [
        (
            json!({"files": ["lib/**"], "deny": ["["], "message": "Invalid glob"}),
            ".deny[0]",
        ),
        (
            json!({"files": ["lib/**", 42], "deny": ["dart:async"], "message": "Invalid selector"}),
            ".files[1]",
        ),
        (
            json!({"files": ["lib/**"], "deny": ["dart:async", false], "message": "Invalid deny"}),
            ".deny[1]",
        ),
        (
            json!({"files": ["lib/**"], "deny": ["dart:async"], "message": " "}),
            ".message",
        ),
        (
            json!({"deny": ["dart:async"], "message": "Missing files"}),
            "",
        ),
    ] {
        fs::write(
            &path,
            json!({
                "cross-file": { "rules": { "correctness": {
                    "banned-imports": { "level": "warn", "options": { "rules": [entry] } }
                } } }
            })
            .to_string(),
        )
        .unwrap();
        let error = load_config(&path).unwrap_err();
        let expected =
            format!("`cross-file.rules.correctness.banned-imports.options.rules[0]{suffix}`");
        assert!(error.0.contains(&expected), "{error}");
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn banned_rules_reject_previously_ignored_invalid_entries() {
    let directory = directory();
    let path = directory.join("falcon.json");
    for rule in [
        "avoid-banned-names",
        "avoid-banned-annotations",
        "avoid-banned-exports",
        "avoid-banned-types",
        "banned-usage",
    ] {
        for (entry, suffix) in [
            (
                json!({"deny": ["*", null], "message": "Invalid mixed list"}),
                ".deny[1]",
            ),
            (
                json!({"deny": ["["], "message": "Invalid glob"}),
                ".deny[0]",
            ),
            (json!({"deny": ["**"], "message": " "}), ".message"),
        ] {
            fs::write(
                &path,
                json!({
                    "linter": { "rules": { "suspicious": {
                        (rule): { "level": "warn", "options": { "rules": [entry] } }
                    } } }
                })
                .to_string(),
            )
            .unwrap();
            let error = load_config(&path).unwrap_err();
            let expected = format!("`linter.rules.suspicious.{rule}.options.rules[0]{suffix}`");
            assert!(error.0.contains(&expected), "{error}");
        }
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn ordering_rules_reject_invalid_order_in_base_and_overrides() {
    let directory = directory();
    let path = directory.join("falcon.json");
    for rule in [
        "arguments-ordering",
        "enum-constants-ordering",
        "initializers-ordering",
        "map-keys-ordering",
        "parameters-ordering",
        "pattern-fields-ordering",
        "record-fields-ordering",
    ] {
        for in_override in [false, true] {
            let rules = json!({ "linter": { "rules": { "style": {
                (rule): { "level": "warn", "options": { "order": "invalid" } }
            } } } });
            let value = if in_override {
                let mut entry = rules;
                entry["includes"] = json!(["lib/**"]);
                json!({ "overrides": [entry] })
            } else {
                rules
            };
            fs::write(&path, value.to_string()).unwrap();
            let error = load_config(&path).unwrap_err();
            let prefix = if in_override { "overrides[0]." } else { "" };
            let expected = format!("{prefix}linter.rules.style.{rule}.options.order");
            assert!(error.0.contains(&expected), "{error}");
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
