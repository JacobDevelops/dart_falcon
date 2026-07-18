use falcon_config::{
    ConfigError, DomainValue, FalconConfig, Override, ResolvedSeverity, RuleConfiguration,
    RulePlainConfiguration, find_config, load_config, load_or_default,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_file_path(name: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!("falcon_test_{}_{}", id, name));
    path
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

fn from_json(value: serde_json::Value) -> FalconConfig {
    serde_json::from_value(value).expect("valid config")
}

// ── schema shape ────────────────────────────────────────────────────────────

#[test]
fn test_default_config() {
    let cfg = FalconConfig::default();
    assert!(cfg.schema.is_none());
    assert!(cfg.files.includes.is_empty());
    assert!(cfg.linter.enabled);
    assert_eq!(cfg.linter.rules.recommended, None);
    assert!(cfg.linter.rules.groups.is_empty());
    assert!(cfg.linter.domains.is_empty());
    assert_eq!(cfg.max_errors, None);
}

#[test]
fn test_parse_full_example() {
    let cfg = from_json(serde_json::json!({
        "$schema": "https://example.invalid/falcon-schema.json",
        "files": {
            "includes": ["**", "!.dart_tool/**", "!build/**", "!**/*.g.dart"]
        },
        "linter": {
            "enabled": true,
            "rules": {
                "recommended": true,
                "complexity": { "max_lines_for_file": "off" },
                "style": { "prefer-trailing-comma": { "level": "error", "options": {} } }
            },
            "domains": { "flutter": "recommended" }
        },
        "max_errors": null
    }));

    assert_eq!(
        cfg.schema.as_deref(),
        Some("https://example.invalid/falcon-schema.json")
    );
    assert_eq!(
        cfg.files.exclude_patterns(),
        vec![
            ".dart_tool/**".to_string(),
            "build/**".to_string(),
            "**/*.g.dart".to_string()
        ]
    );
    // "**" present → no positive filtering.
    assert!(cfg.files.include_patterns().is_empty());
    assert!(cfg.linter.enabled);
    assert_eq!(cfg.linter.rules.recommended, Some(true));
    assert_eq!(
        cfg.linter.domains.get("flutter"),
        Some(&DomainValue::Recommended)
    );

    let complexity = cfg.linter.rules.groups.get("complexity").unwrap();
    assert_eq!(
        complexity.get("max_lines_for_file").unwrap().level(),
        RulePlainConfiguration::Off
    );
    let style = cfg.linter.rules.groups.get("style").unwrap();
    assert_eq!(
        style.get("prefer-trailing-comma").unwrap().level(),
        RulePlainConfiguration::Error
    );
}

#[test]
fn test_rule_configuration_both_forms() {
    // Plain string form.
    let plain: RuleConfiguration = serde_json::from_value(serde_json::json!("warn")).unwrap();
    assert!(matches!(plain, RuleConfiguration::Plain(_)));
    assert_eq!(plain.level(), RulePlainConfiguration::Warn);
    assert!(plain.options().is_none());

    // Object form with options.
    let with_options: RuleConfiguration =
        serde_json::from_value(serde_json::json!({ "level": "error", "options": { "k": 1 } }))
            .unwrap();
    assert_eq!(with_options.level(), RulePlainConfiguration::Error);
    assert_eq!(
        with_options
            .options()
            .and_then(|o| o.get("k"))
            .and_then(|v| v.as_i64()),
        Some(1)
    );

    // Object form with omitted options normalizes to None, like the plain form.
    let no_options: RuleConfiguration =
        serde_json::from_value(serde_json::json!({ "level": "on" })).unwrap();
    assert_eq!(no_options.level(), RulePlainConfiguration::On);
    assert!(no_options.options().is_none());
}

#[test]
fn test_invalid_rule_level_names_offending_value() {
    // Plain string form: a typo'd level names the value and the valid set.
    let err = serde_json::from_value::<RuleConfiguration>(serde_json::json!("warning"))
        .expect_err("invalid level must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("warning"),
        "error should name the value: {msg}"
    );
    assert!(
        msg.contains("off, on, info, warn, error"),
        "error should list valid levels: {msg}"
    );

    // Object form: the nested level is validated the same way.
    let err = serde_json::from_value::<RuleConfiguration>(serde_json::json!({ "level": "errr" }))
        .expect_err("invalid nested level must be rejected");
    let msg = err.to_string();
    assert!(msg.contains("errr"), "error should name the value: {msg}");
    assert!(
        msg.contains("off, on, info, warn, error"),
        "error should list valid levels: {msg}"
    );
}

#[test]
fn test_files_include_patterns_semantics() {
    let cfg = from_json(serde_json::json!({
        "files": { "includes": ["src/**", "lib/**", "!lib/gen/**"] }
    }));
    assert_eq!(cfg.files.exclude_patterns(), vec!["lib/gen/**".to_string()]);
    assert_eq!(
        cfg.files.include_patterns(),
        vec!["src/**".to_string(), "lib/**".to_string()]
    );
}

// ── resolution matrix ─────────────────────────────────────────────────────────

#[test]
fn test_resolve_default_everything_on_at_warning() {
    let cfg = FalconConfig::default();
    // No domains, recommended rule → Warn.
    assert_eq!(
        cfg.resolve_rule("suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Warn)
    );
    // Flutter rule, missing domain key defaults to Recommended → enabled.
    assert_eq!(
        cfg.resolve_rule("style", "use-design-system-item", true, &["flutter"]),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn test_resolve_explicit_levels() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "suspicious": {
            "off-rule": "off",
            "on-rule": "on",
            "info-rule": "info",
            "warn-rule": "warn",
            "error-rule": "error"
        } } }
    }));
    assert_eq!(cfg.resolve_rule("suspicious", "off-rule", true, &[]), None);
    assert_eq!(
        cfg.resolve_rule("suspicious", "on-rule", true, &[]),
        Some(ResolvedSeverity::Warn)
    );
    assert_eq!(
        cfg.resolve_rule("suspicious", "info-rule", true, &[]),
        Some(ResolvedSeverity::Info)
    );
    assert_eq!(
        cfg.resolve_rule("suspicious", "warn-rule", true, &[]),
        Some(ResolvedSeverity::Warn)
    );
    assert_eq!(
        cfg.resolve_rule("suspicious", "error-rule", true, &[]),
        Some(ResolvedSeverity::Error)
    );
}

#[test]
fn test_resolve_recommended_false_disables_non_explicit() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": false } }
    }));
    // Non-explicit recommended rule is off when the preset is disabled.
    assert_eq!(
        cfg.resolve_rule("suspicious", "avoid-dynamic", true, &[]),
        None
    );
    // A non-recommended rule is likewise off.
    assert_eq!(cfg.resolve_rule("style", "some-rule", false, &[]), None);
}

#[test]
fn test_resolve_explicit_entry_beats_recommended_false() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": {
            "recommended": false,
            "suspicious": { "avoid-dynamic": "error" }
        } }
    }));
    assert_eq!(
        cfg.resolve_rule("suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Error)
    );
}

#[test]
fn test_resolve_domains() {
    // flutter = none disables a flutter rule with no explicit entry.
    let none = from_json(serde_json::json!({
        "linter": { "domains": { "flutter": "none" } }
    }));
    assert_eq!(
        none.resolve_rule("style", "use-design-system-item", true, &["flutter"]),
        None
    );
    // ...but a non-flutter rule stays on.
    assert_eq!(
        none.resolve_rule("suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Warn)
    );

    // flutter = all enables even when recommended preset is off.
    let all = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": false }, "domains": { "flutter": "all" } }
    }));
    assert_eq!(
        all.resolve_rule("style", "use-design-system-item", true, &["flutter"]),
        Some(ResolvedSeverity::Warn)
    );

    // flutter = recommended tracks the recommended preset.
    let rec = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": false }, "domains": { "flutter": "recommended" } }
    }));
    assert_eq!(
        rec.resolve_rule("style", "use-design-system-item", true, &["flutter"]),
        None
    );
}

#[test]
fn test_resolve_explicit_entry_bypasses_domain_gating() {
    // Domain gated off, but the rule is explicitly enabled → wins.
    let cfg = from_json(serde_json::json!({
        "linter": {
            "rules": { "style": { "use-design-system-item": "warn" } },
            "domains": { "flutter": "none" }
        }
    }));
    assert_eq!(
        cfg.resolve_rule("style", "use-design-system-item", true, &["flutter"]),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn test_resolve_linter_disabled_disables_everything() {
    let cfg = from_json(serde_json::json!({
        "linter": {
            "enabled": false,
            "rules": { "suspicious": { "avoid-dynamic": "error" } }
        }
    }));
    assert_eq!(
        cfg.resolve_rule("suspicious", "avoid-dynamic", true, &[]),
        None
    );
}

#[test]
fn test_rule_options_scoped_to_group() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "style": {
            "use-design-system-item": { "level": "warn", "options": { "items": [1, 2] } }
        } } }
    }));
    let opts = cfg
        .rule_options("style", "use-design-system-item")
        .expect("options present");
    assert_eq!(
        opts.get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(2)
    );
    // Plain form or absent rule → no options.
    assert!(cfg.rule_options("suspicious", "avoid-dynamic").is_none());
}

#[test]
fn test_rule_options_ignores_wrong_group_entry() {
    // `use-design-system-item` belongs to `style`, but here it is misplaced
    // under `complexity`. Options must NOT be applied (lookup is group-scoped)
    // and the rule must still resolve at its default severity — matching how
    // `resolve_rule` ignores the misplaced entry's level.
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "complexity": {
            "use-design-system-item": { "level": "off", "options": { "items": [1, 2] } }
        } } }
    }));
    // Options are ignored when queried under the rule's real group.
    assert!(
        cfg.rule_options("style", "use-design-system-item")
            .is_none()
    );
    // And the misplaced `"off"` does not disable it: still on at default (Warn).
    assert_eq!(
        cfg.resolve_rule("style", "use-design-system-item", true, &["flutter"]),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn test_include_patterns_catch_all_star_star_slash_star() {
    // `**/*` is treated the same as `**`: no positive filtering.
    let cfg = from_json(serde_json::json!({
        "files": { "includes": ["**/*", "!build/**"] }
    }));
    assert!(cfg.files.include_patterns().is_empty());
    assert_eq!(cfg.files.exclude_patterns(), vec!["build/**".to_string()]);
}

// ── loading & discovery ───────────────────────────────────────────────────────

#[test]
fn test_load_valid_config() {
    let path = temp_file_path("valid_config.json");
    let json = r#"{
        "files": { "includes": ["**", "!*.tmp"] },
        "linter": { "rules": { "recommended": true } },
        "max_errors": 100
    }"#;
    fs::write(&path, json).expect("write temp file");

    let cfg = load_config(&path).expect("load valid config");
    assert_eq!(cfg.files.exclude_patterns(), vec!["*.tmp".to_string()]);
    assert_eq!(cfg.max_errors, Some(100));

    cleanup(&path);
}

#[test]
fn test_load_empty_object_is_all_defaults() {
    let path = temp_file_path("empty.json");
    fs::write(&path, "{}").expect("write temp file");

    let cfg = load_config(&path).expect("load empty config");
    assert!(cfg.linter.enabled);
    assert_eq!(cfg.max_errors, None);

    cleanup(&path);
}

#[test]
fn test_load_invalid_json() {
    let path = temp_file_path("invalid_json.json");
    fs::write(&path, "{ invalid json }").expect("write temp file");
    assert!(load_config(&path).is_err());
    cleanup(&path);
}

#[test]
fn test_load_missing_file() {
    let path = temp_file_path("nonexistent.json");
    assert!(load_config(&path).is_err());
}

#[test]
fn test_legacy_schema_is_rejected() {
    for legacy in [
        r#"{ "rules": { "avoid-dynamic": { "enabled": false } } }"#,
        r#"{ "exclude_patterns": ["**/gen/**"] }"#,
        r#"{ "severity_override": { "avoid-dynamic": "error" } }"#,
    ] {
        let path = temp_file_path("legacy.json");
        fs::write(&path, legacy).expect("write temp file");
        let err = load_config(&path).expect_err("legacy schema must be rejected");
        assert!(
            err.to_string().contains("legacy flat schema"),
            "error should mention legacy schema: {err}"
        );
        cleanup(&path);
    }
}

#[test]
fn test_find_config_in_cwd() {
    let dir = std::env::temp_dir();
    let config_path = dir.join("falcon.json");
    fs::write(&config_path, "{}").expect("write config");

    let found = find_config(&dir);
    assert_eq!(found, Some(config_path.clone()));

    fs::remove_file(&config_path).expect("cleanup");
}

#[test]
fn test_find_config_none_when_no_local_or_git_config() {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("falcon_test_no_config_{}", id));
    fs::create_dir_all(&dir).expect("create temp dir");
    assert!(!dir.join("falcon.json").exists());

    let found = find_config(&dir);
    let home_config_exists = std::env::var("HOME")
        .map(|h| std::path::Path::new(&h).join(".falcon.json").exists())
        .unwrap_or(false);
    if !home_config_exists {
        assert!(found.is_none(), "expected no config but got: {:?}", found);
    }

    let _ = fs::remove_dir(&dir);
}

#[test]
fn test_config_error_display() {
    let err = ConfigError("test error message".to_string());
    assert!(err.to_string().contains("test error message"));
}

#[test]
fn test_load_or_default_missing() {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("falcon_test_load_or_default_{}", id));
    fs::create_dir_all(&dir).expect("create temp dir");

    let cfg = load_or_default(&dir);
    assert!(cfg.linter.enabled);
    assert_eq!(cfg.max_errors, None);

    let _ = fs::remove_dir(&dir);
}

#[test]
fn test_load_or_default_with_file() {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("falcon_test_load_or_default_file_{}", id));
    fs::create_dir_all(&dir).expect("create temp dir");

    let config_path = dir.join("falcon.json");
    let json = r#"{ "linter": { "rules": { "recommended": true } }, "max_errors": 50 }"#;
    fs::write(&config_path, json).expect("write config");

    let cfg = load_or_default(&dir);
    assert_eq!(cfg.linter.rules.recommended, Some(true));
    assert_eq!(cfg.max_errors, Some(50));

    fs::remove_file(&config_path).expect("cleanup config");
    let _ = fs::remove_dir(&dir);
}

// ── overrides (biome-style per-path re-configuration) ───────────────────────

fn override_from(value: serde_json::Value) -> Override {
    serde_json::from_value(value).expect("valid override")
}

#[test]
fn test_overrides_serde_round_trip() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": true } },
        "overrides": [
            {
                "includes": ["test/**", "!test/fixtures/**"],
                "linter": { "rules": { "complexity": { "max_lines_for_function": "off" } } }
            }
        ]
    }));
    assert_eq!(cfg.overrides.len(), 1);
    assert_eq!(
        cfg.overrides[0].includes,
        vec!["test/**".to_string(), "!test/fixtures/**".to_string()]
    );

    // Round-trip through serialization preserves the overrides.
    let json = serde_json::to_value(&cfg).expect("serialize");
    let back: FalconConfig = serde_json::from_value(json).expect("reparse");
    assert_eq!(back.overrides.len(), 1);
    assert_eq!(back.overrides[0].includes, cfg.overrides[0].includes);
}

#[test]
fn test_default_config_has_no_overrides() {
    assert!(FalconConfig::default().overrides.is_empty());
}

#[test]
fn test_override_matches_positive_and_negation() {
    let ov = override_from(serde_json::json!({
        "includes": ["test/**", "!test/fixtures/**"]
    }));
    assert!(ov.matches("test/foo.dart"));
    assert!(ov.matches("test/sub/foo.dart"));
    assert!(
        !ov.matches("test/fixtures/bar.dart"),
        "negation must exclude"
    );
    assert!(!ov.matches("lib/foo.dart"), "non-matching path excluded");
}

#[test]
fn test_resolve_rule_for_base_on_override_off() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": true } },
        "overrides": [ {
            "includes": ["test/**"],
            "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } }
        } ]
    }));
    // Sanity: base resolution is on.
    assert_eq!(
        cfg.resolve_rule("suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Warn)
    );
    // The override turns it off only for matching paths.
    assert_eq!(
        cfg.resolve_rule_for("test/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        None
    );
    assert_eq!(
        cfg.resolve_rule_for("lib/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn test_resolve_rule_for_base_off_override_on_with_severity() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } },
        "overrides": [ {
            "includes": ["test/**"],
            "linter": { "rules": { "suspicious": { "avoid-dynamic": "error" } } }
        } ]
    }));
    assert_eq!(
        cfg.resolve_rule_for("test/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Error)
    );
    assert_eq!(
        cfg.resolve_rule_for("lib/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        None
    );
    // Registration must include a rule an override re-enables.
    assert!(cfg.is_rule_enabled_anywhere("suspicious", "avoid-dynamic", true, &[]));
}

#[test]
fn test_resolve_rule_for_later_override_wins() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": true } },
        "overrides": [
            { "includes": ["test/**"],
              "linter": { "rules": { "suspicious": { "avoid-dynamic": "error" } } } },
            { "includes": ["test/**"],
              "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } } }
        ]
    }));
    assert_eq!(
        cfg.resolve_rule_for("test/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        None,
        "later override wins"
    );
}

#[test]
fn test_override_enabled_false_disables_all_rules_for_path() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "recommended": true } },
        "overrides": [ { "includes": ["gen/**"], "linter": { "enabled": false } } ]
    }));
    assert_eq!(
        cfg.resolve_rule_for("gen/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        None
    );
    assert_eq!(
        cfg.resolve_rule_for("lib/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn test_globally_disabled_linter_not_resurrected_by_override() {
    let cfg = from_json(serde_json::json!({
        "linter": {
            "enabled": false,
            "rules": { "recommended": true }
        },
        "overrides": [ {
            "includes": ["test/**"],
            "linter": { "rules": { "suspicious": { "avoid-dynamic": "error" } } }
        } ]
    }));
    assert_eq!(
        cfg.resolve_rule_for("test/a.dart", "suspicious", "avoid-dynamic", true, &[]),
        None
    );
    assert!(!cfg.is_rule_enabled_anywhere("suspicious", "avoid-dynamic", true, &[]));
}

// ── project (cross-file) rules ──────────────────────────────────────────────

#[test]
fn test_project_default_enabled_and_recommended() {
    let cfg = FalconConfig::default();
    assert!(cfg.project.enabled);
    // Recommended project rule → Warn by default.
    assert_eq!(
        cfg.project
            .resolve_rule("correctness", "unused-files", true),
        Some(ResolvedSeverity::Warn)
    );
    // Non-recommended project rule (e.g. unnecessary-nullable) → off by default.
    assert_eq!(
        cfg.project
            .resolve_rule("correctness", "unnecessary-nullable", false),
        None
    );
}

#[test]
fn test_project_explicit_levels_and_disabled_switch() {
    let cfg = from_json(serde_json::json!({
        "project": { "rules": { "correctness": {
            "unused-files": "error",
            "unnecessary-nullable": "warn"
        } } }
    }));
    assert_eq!(
        cfg.resolve_project_rule_for("lib/a.dart", "correctness", "unused-files", true),
        Some(ResolvedSeverity::Error)
    );
    // Explicit entry beats the recommended=false gate.
    assert_eq!(
        cfg.resolve_project_rule_for("lib/a.dart", "correctness", "unnecessary-nullable", false),
        Some(ResolvedSeverity::Warn)
    );

    // project.enabled=false kills every project rule.
    let off = from_json(serde_json::json!({
        "project": { "enabled": false, "rules": { "correctness": { "unused-files": "error" } } }
    }));
    assert_eq!(
        off.resolve_project_rule_for("lib/a.dart", "correctness", "unused-files", true),
        None
    );
    assert!(!off.is_project_rule_enabled_anywhere("correctness", "unused-files", true));
}

#[test]
fn test_project_overrides_patch_per_path() {
    let cfg = from_json(serde_json::json!({
        "overrides": [ {
            "includes": ["test/**"],
            "project": { "rules": { "correctness": { "unused-files": "off" } } }
        } ]
    }));
    // Base (recommended) resolution is on...
    assert_eq!(
        cfg.resolve_project_rule_for("lib/a.dart", "correctness", "unused-files", true),
        Some(ResolvedSeverity::Warn)
    );
    // ...but the override turns it off for matching paths.
    assert_eq!(
        cfg.resolve_project_rule_for("test/a.dart", "correctness", "unused-files", true),
        None
    );

    // Base off, override re-enables → registration must include it.
    let reenable = from_json(serde_json::json!({
        "project": { "rules": { "correctness": { "unused-files": "off" } } },
        "overrides": [ {
            "includes": ["test/**"],
            "project": { "rules": { "correctness": { "unused-files": "error" } } }
        } ]
    }));
    assert_eq!(
        reenable.resolve_project_rule_for("test/a.dart", "correctness", "unused-files", true),
        Some(ResolvedSeverity::Error)
    );
    assert!(reenable.is_project_rule_enabled_anywhere("correctness", "unused-files", true));

    // An override's project.enabled=false disables all project rules for the path.
    let disabled = from_json(serde_json::json!({
        "overrides": [ { "includes": ["gen/**"], "project": { "enabled": false } } ]
    }));
    assert_eq!(
        disabled.resolve_project_rule_for("gen/a.dart", "correctness", "unused-files", true),
        None
    );
    assert_eq!(
        disabled.resolve_project_rule_for("lib/a.dart", "correctness", "unused-files", true),
        Some(ResolvedSeverity::Warn)
    );
}

#[test]
fn test_options_in_override_load_ok() {
    // Options inside an override used to be rejected at load; they are now a
    // supported per-path feature and must load cleanly.
    let path = temp_file_path("override_options.json");
    fs::write(
        &path,
        r#"{
            "overrides": [ {
                "includes": ["test/**"],
                "linter": { "rules": { "complexity": {
                    "max_lines_for_file": { "level": "warn", "options": { "max_lines": 10 } }
                } } },
                "project": { "rules": { "correctness": {
                    "unused-files": { "level": "warn", "options": { "k": 1 } }
                } } }
            } ]
        }"#,
    )
    .expect("write config");
    load_config(&path).expect("override with options must load");
    cleanup(&path);
}

#[test]
fn test_override_options_replace_base_per_path() {
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "complexity": {
            "max_lines_for_file": { "level": "warn", "options": { "max_lines": 400 } }
        } } },
        "overrides": [ {
            "includes": ["test/**"],
            "linter": { "rules": { "complexity": {
                "max_lines_for_file": { "level": "warn", "options": { "max_lines": 10 } }
            } } }
        } ]
    }));

    // Non-matching path: base options.
    assert_eq!(
        cfg.rule_options_for("lib/main.dart", "complexity", "max_lines_for_file")
            .and_then(|o| o.get("max_lines"))
            .and_then(|v| v.as_u64()),
        Some(400)
    );
    // Matching path: override options replace the base wholesale.
    assert_eq!(
        cfg.rule_options_for("test/widget_test.dart", "complexity", "max_lines_for_file")
            .and_then(|o| o.get("max_lines"))
            .and_then(|v| v.as_u64()),
        Some(10)
    );
}

#[test]
fn test_override_without_options_keeps_base_options() {
    // An override that changes only the level leaves the base options intact.
    let cfg = from_json(serde_json::json!({
        "linter": { "rules": { "complexity": {
            "max_lines_for_file": { "level": "warn", "options": { "max_lines": 400 } }
        } } },
        "overrides": [ {
            "includes": ["test/**"],
            "linter": { "rules": { "complexity": { "max_lines_for_file": "error" } } }
        } ]
    }));
    assert_eq!(
        cfg.rule_options_for("test/widget_test.dart", "complexity", "max_lines_for_file")
            .and_then(|o| o.get("max_lines"))
            .and_then(|v| v.as_u64()),
        Some(400)
    );
}
