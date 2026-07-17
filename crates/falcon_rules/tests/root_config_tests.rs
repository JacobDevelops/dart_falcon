use std::collections::BTreeSet;
use std::path::Path;

use falcon_rules::meta::{RULE_METADATA, meta_for};

/// Collect the `<group>.<rule>` names listed under a config section object
/// (`linter.rules` or `cross_file.rules`), asserting each names a known rule placed
/// under its correct group. Returns the set of listed rule names.
fn listed_rules(section: &serde_json::Value, section_name: &str) -> BTreeSet<&'static str> {
    let groups = section
        .as_object()
        .unwrap_or_else(|| panic!("{section_name} must be an object"));
    let mut listed = BTreeSet::new();
    for (group, rules) in groups {
        if group == "recommended" {
            continue;
        }
        for (name, _) in rules.as_object().expect("group is an object") {
            let meta = meta_for(name)
                .unwrap_or_else(|| panic!("{section_name} lists unknown rule `{name}`"));
            assert_eq!(
                meta.group, group,
                "{section_name} places `{name}` under `{group}` but meta says `{}`",
                meta.group
            );
            listed.insert(meta.name);
        }
    }
    listed
}

// The root falcon.json is the showcase config: every rule listed explicitly
// under its section (file rules under `linter.rules`, cross-file rules under
// `cross_file.rules`), grouped by category. Keep it in lockstep with the registry
// so new rules can't silently miss the showcase — and so neither section leaks
// the other kind of rule.
#[test]
fn root_falcon_json_splits_file_and_cross_file_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../falcon.json");
    let raw = std::fs::read_to_string(root).expect("root falcon.json readable");
    let json: serde_json::Value =
        serde_json::from_str(&raw).expect("root falcon.json is valid JSON");

    let linter_listed = listed_rules(&json["linter"]["rules"], "linter.rules");
    let cross_file_listed = listed_rules(&json["cross-file"]["rules"], "cross_file.rules");

    // Neither section may contain the other kind of rule.
    for name in &linter_listed {
        assert!(
            !meta_for(name).unwrap().cross_file,
            "linter.rules must not list cross-file rule `{name}`"
        );
    }
    for name in &cross_file_listed {
        assert!(
            meta_for(name).unwrap().cross_file,
            "cross_file.rules must not list file rule `{name}`"
        );
    }

    // Every file rule appears under linter.rules; every cross-file rule under
    // cross_file.rules. No rule is listed in both.
    let missing_file: Vec<_> = RULE_METADATA
        .iter()
        .filter(|m| !m.cross_file && !linter_listed.contains(m.name))
        .map(|m| m.name)
        .collect();
    assert!(
        missing_file.is_empty(),
        "file rules missing from linter.rules showcase: {missing_file:?}"
    );
    let missing_cross_file: Vec<_> = RULE_METADATA
        .iter()
        .filter(|m| m.cross_file && !cross_file_listed.contains(m.name))
        .map(|m| m.name)
        .collect();
    assert!(
        missing_cross_file.is_empty(),
        "cross-file rules missing from cross_file.rules showcase: {missing_cross_file:?}"
    );
    assert!(
        linter_listed.is_disjoint(&cross_file_listed),
        "a rule is listed in both linter.rules and cross_file.rules"
    );
    assert_eq!(
        linter_listed.len() + cross_file_listed.len(),
        RULE_METADATA.len(),
        "showcase must list every rule exactly once across both sections"
    );
}
