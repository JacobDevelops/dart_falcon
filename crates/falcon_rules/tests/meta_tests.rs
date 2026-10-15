//! Metadata, schema, and legacy configuration contracts.

use std::collections::HashSet;

use falcon_rules::meta::{RULE_METADATA, RuleSource, meta_for};

#[test]
fn registry_resolution_requirement_uses_registered_names() {
    use falcon_analyze::RuleRegistry;
    use falcon_rules::meta::RuleKind;
    let mut registry = RuleRegistry::new();
    assert!(!falcon_rules::requires_resolution(&registry));
    for (name, expected) in [
        ("avoid-dynamic", false),
        ("avoid-ignoring-return-values", true),
    ] {
        let RuleKind::File { make, .. } = meta_for(name).unwrap().kind else {
            panic!("expected file rule");
        };
        registry.register(name, make());
        assert_eq!(falcon_rules::requires_resolution(&registry), expected);
    }
    assert!(!falcon_rules::rule_requires_resolution("unknown-rule"));
}

#[test]
fn domain_metadata_preserves_configuration_names() {
    for meta in RULE_METADATA {
        let expected: Vec<_> = meta.domain.iter().map(|domain| domain.as_str()).collect();
        assert_eq!(meta.domain_names(), expected);
    }
}

#[test]
fn committed_schema_matches_generator() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/falcon.schema.json"
    );
    let committed = std::fs::read_to_string(path)
        .expect("schema/falcon.schema.json exists — run `cargo xtask schema`");
    assert_eq!(
        committed,
        falcon_rules::schema::config_schema_string(),
        "schema/falcon.schema.json is stale; run `cargo xtask schema`"
    );
}

#[test]
fn metadata_names_are_unique() {
    let mut seen = HashSet::new();
    for meta in RULE_METADATA {
        assert!(
            seen.insert(meta.name),
            "duplicate metadata entry: {}",
            meta.name
        );
    }
}

#[test]
fn every_rule_has_a_source_with_a_non_empty_upstream_id() {
    for meta in RULE_METADATA {
        match meta.source {
            RuleSource::DartCodeLinter(id)
            | RuleSource::PyramidLint(id)
            | RuleSource::Lints(id) => assert!(
                !id.is_empty(),
                "rule `{}` has a ported source with an empty upstream id",
                meta.name
            ),
            RuleSource::Falcon => {}
        }
    }
}

#[test]
fn meta_for_finds_known_and_rejects_unknown() {
    assert!(meta_for("avoid-dynamic").is_some());
    assert!(meta_for("this-rule-does-not-exist").is_none());
}

#[test]
fn legacy_aliases_resolve_to_canonical_metadata() {
    use falcon_rules::meta::{RULE_ALIASES, canonical_rule_name};

    // Every alias maps to a real canonical rule, and its canonical id is not
    // itself an alias key (no chains).
    for (old, canonical) in RULE_ALIASES {
        assert_eq!(canonical_rule_name(old), *canonical, "alias {old}");
        assert!(
            meta_for(old).is_some_and(|m| m.name == *canonical),
            "alias `{old}` must resolve to `{canonical}`"
        );
        assert!(
            !RULE_ALIASES.iter().any(|(k, _)| k == canonical),
            "canonical id `{canonical}` must not also be an alias key"
        );
    }
}

#[test]
fn canonicalize_config_rewrites_legacy_rule_keys() {
    use falcon_config::FalconConfig;
    use falcon_rules::meta::canonicalize_config;

    // A pre-1.0 config using a snake_case id and a removed twin id still turns
    // the canonical rules on after canonicalization.
    let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
        "linter": {
            "rules": {
                "complexity": { "max_lines_for_file": "error" },
                "style": { "no_magic_number": "warn" }
            }
        }
    }))
    .expect("valid config");

    canonicalize_config(&mut config);

    assert_eq!(
        config.resolve_rule("complexity", "max-lines-for-file", false, &[]),
        Some(falcon_config::ResolvedSeverity::Error),
        "legacy `max_lines_for_file` key must enable `max-lines-for-file`"
    );
    assert_eq!(
        config.resolve_rule("style", "no-magic-number", false, &[]),
        Some(falcon_config::ResolvedSeverity::Warn),
        "removed twin `no_magic_number` key must enable `no-magic-number`"
    );
}

#[test]
fn canonicalization_preserves_collisions_unknown_keys_and_all_override_maps() {
    use falcon_config::{FalconConfig, RulePlainConfiguration};
    use falcon_rules::meta::canonicalize_config;

    for (first, second) in [("error", "warn"), ("off", "error")] {
        let rules = serde_json::json!({
            "suspicious": {
                "avoid_empty_blocks": first,
                "no_empty_block": second,
                "no-empty-block": "info",
                "unknown-rule": "off"
            }
        });
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": { "rules": rules },
            "cross-file": { "rules": rules },
            "overrides": [{
                "includes": ["test/**"],
                "linter": { "rules": rules },
                "cross-file": { "rules": rules }
            }]
        }))
        .expect("valid config");
        canonicalize_config(&mut config);
        let once = serde_json::to_value(&config).expect("serializable config");
        canonicalize_config(&mut config);
        assert_eq!(serde_json::to_value(&config).unwrap(), once);
        for rules in [
            &config.linter.rules,
            &config.cross_file.rules,
            &config.overrides[0].linter.as_ref().unwrap().rules,
            &config.overrides[0].cross_file.as_ref().unwrap().rules,
        ] {
            let group = &rules.groups["suspicious"];
            assert_eq!(group.len(), 2);
            assert_eq!(
                group["no-empty-block"].level(),
                RulePlainConfiguration::Error
            );
            assert_eq!(group["unknown-rule"].level(), RulePlainConfiguration::Off);
        }
    }
}

#[test]
fn equal_severity_collisions_preserve_canonical_options_then_first_legacy_options() {
    use falcon_config::FalconConfig;
    use falcon_rules::meta::canonicalize_config;

    for canonical in [false, true] {
        let first = serde_json::json!({ "level": "warn", "options": { "winner": "first" } });
        let canonical_config =
            serde_json::json!({ "level": "on", "options": { "winner": "canonical" } });
        let mut group = serde_json::json!({
            "avoid_empty_blocks": first,
            "no_empty_block": { "level": "warn", "options": { "winner": "second" } }
        });
        if canonical {
            group["no-empty-block"] = canonical_config.clone();
        }
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": { "rules": { "suspicious": group } }
        }))
        .unwrap();
        canonicalize_config(&mut config);
        let result = &config.linter.rules.groups["suspicious"];
        assert_eq!(result.len(), 1);
        assert_eq!(
            serde_json::to_value(&result["no-empty-block"]).unwrap(),
            if canonical { canonical_config } else { first }
        );
    }
}
