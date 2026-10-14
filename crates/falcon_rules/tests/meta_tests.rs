//! Metadata, schema, and legacy configuration contracts.

use std::collections::HashSet;

use falcon_rules::meta::{RULE_METADATA, RuleSource, meta_for};

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
fn canonicalize_config_merges_twins_keeping_more_severe_level() {
    use falcon_config::{FalconConfig, ResolvedSeverity};
    use falcon_rules::meta::canonicalize_config;

    // Both legacy empty-block twins configured at different levels collapse to
    // the surviving `no-empty-block`; the more severe level wins regardless of
    // iteration order.
    let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
        "linter": {
            "rules": {
                "suspicious": {
                    "avoid_empty_blocks": "off",
                    "no_empty_block": "error"
                }
            }
        }
    }))
    .expect("valid config");

    canonicalize_config(&mut config);

    assert_eq!(
        config.resolve_rule("suspicious", "no-empty-block", false, &[]),
        Some(ResolvedSeverity::Error),
        "merged twins must keep the more severe level"
    );
}
