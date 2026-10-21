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
