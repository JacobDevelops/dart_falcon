//! Contract tests binding the rule metadata table to the registered rule set.
//!
//! Every rule in `all_rules()` must have exactly one metadata entry, every
//! metadata entry must match a registered rule, and every group must be one of
//! the known categories. This replaces the earlier "falcon.json enumerates
//! every rule" contract: the metadata table is now the single source of truth
//! that ties each rule to the biome-style config schema.

use std::collections::HashSet;

use falcon_rules::meta::{DOMAINS, GROUPS, RULE_METADATA, RuleSource, meta_for};
use falcon_rules::{all_project_rules, all_rules};

/// Every registered rule name — per-file rules plus project (cross-file) rules,
/// which share the metadata table and config schema.
fn registered_rule_names() -> Vec<&'static str> {
    all_rules()
        .iter()
        .map(|r| r.name())
        .chain(all_project_rules().iter().map(|r| r.name()))
        .collect()
}

#[test]
fn every_registered_rule_has_exactly_one_metadata_entry() {
    for name in registered_rule_names() {
        let matches = RULE_METADATA.iter().filter(|m| m.name == name).count();
        assert_eq!(
            matches, 1,
            "rule `{name}` must have exactly one metadata entry, found {matches}"
        );
    }
}

#[test]
fn every_metadata_entry_matches_a_registered_rule() {
    let registered: HashSet<&str> = registered_rule_names().into_iter().collect();
    for meta in RULE_METADATA {
        assert!(
            registered.contains(meta.name),
            "metadata entry `{}` matches no registered rule",
            meta.name
        );
    }
}

#[test]
fn metadata_groups_are_from_the_known_set() {
    let known: HashSet<&str> = GROUPS.iter().copied().collect();
    for meta in RULE_METADATA {
        assert!(
            known.contains(meta.group),
            "rule `{}` has unknown group `{}`",
            meta.name,
            meta.group
        );
    }
}

#[test]
fn metadata_domains_are_from_the_known_set() {
    let known: HashSet<&str> = DOMAINS.iter().copied().collect();
    for meta in RULE_METADATA {
        for domain in meta.domains {
            assert!(
                known.contains(domain),
                "rule `{}` has unknown domain `{}`",
                meta.name,
                domain
            );
        }
    }
}

#[test]
fn metadata_count_matches_registered_rule_count() {
    assert_eq!(
        RULE_METADATA.len(),
        all_rules().len() + all_project_rules().len(),
        "metadata table must match the per-file plus project rule count"
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
fn project_flag_matches_the_project_rule_set() {
    let project_rule_names: HashSet<&str> = all_project_rules().iter().map(|r| r.name()).collect();
    let file_rule_names: HashSet<&str> = all_rules().iter().map(|r| r.name()).collect();

    for meta in RULE_METADATA {
        if project_rule_names.contains(meta.name) {
            assert!(
                meta.project,
                "project rule `{}` must have project=true",
                meta.name
            );
        }
        if file_rule_names.contains(meta.name) {
            assert!(
                !meta.project,
                "file rule `{}` must have project=false",
                meta.name
            );
        }
    }

    // Exactly the three known project rules carry the flag.
    let flagged: HashSet<&str> = RULE_METADATA
        .iter()
        .filter(|m| m.project)
        .map(|m| m.name)
        .collect();
    assert_eq!(
        flagged, project_rule_names,
        "project=true set must equal all_project_rules()"
    );
}

#[test]
fn every_rule_has_a_source_with_a_non_empty_upstream_id() {
    for meta in RULE_METADATA {
        match meta.source {
            RuleSource::DartCodeLinter(id) | RuleSource::PyramidLint(id) => assert!(
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
