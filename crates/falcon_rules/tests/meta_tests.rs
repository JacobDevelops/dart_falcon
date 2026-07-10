//! Contract tests binding the rule metadata table to the registered rule set.
//!
//! Every rule in `all_rules()` must have exactly one metadata entry, every
//! metadata entry must match a registered rule, and every group must be one of
//! the known categories. This replaces the earlier "falcon.json enumerates
//! every rule" contract: the metadata table is now the single source of truth
//! that ties each rule to the biome-style config schema.

use std::collections::HashSet;

use falcon_rules::all_rules;
use falcon_rules::meta::{DOMAINS, GROUPS, RULE_METADATA, meta_for};

#[test]
fn every_registered_rule_has_exactly_one_metadata_entry() {
    for rule in all_rules() {
        let name = rule.name();
        let matches = RULE_METADATA.iter().filter(|m| m.name == name).count();
        assert_eq!(
            matches, 1,
            "rule `{name}` must have exactly one metadata entry, found {matches}"
        );
    }
}

#[test]
fn every_metadata_entry_matches_a_registered_rule() {
    let registered: HashSet<&str> = all_rules().iter().map(|r| r.name()).collect();
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
        all_rules().len(),
        "metadata table and all_rules() must have the same length"
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
fn meta_for_finds_known_and_rejects_unknown() {
    assert!(meta_for("avoid-dynamic").is_some());
    assert!(meta_for("this-rule-does-not-exist").is_none());
}
