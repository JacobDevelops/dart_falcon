use falcon_config::FalconConfig;
use falcon_rules::meta::{RuleKind, meta_for};
use serde_json::json;

use super::ProjectAnalysis;

const STRUCTURAL: &str = "avoid-dynamic";
const SEMANTIC: &str = "avoid-ignoring-return-values";
const CROSS_FILE: &str = "unused-files";
const RESOLVED_CROSS_FILE: &str = "unnecessary-nullable";

fn config_for(file_rules: &[&str], cross_file_rules: &[&str]) -> FalconConfig {
    let mut value = json!({
        "linter": {"rules": {"recommended": false, "suspicious": {}}},
        "cross-file": {"rules": {"recommended": false, "correctness": {}}}
    });
    for name in file_rules {
        value["linter"]["rules"]["suspicious"][*name] = json!("warn");
    }
    for name in cross_file_rules {
        value["cross-file"]["rules"]["correctness"][*name] = json!("warn");
    }
    serde_json::from_value(value).expect("isolated rule configuration is valid")
}

fn plan_for(file_rules: &[&str], cross_file_rules: &[&str]) -> ProjectAnalysis {
    let analysis = ProjectAnalysis::new(&config_for(file_rules, cross_file_rules));
    let mut actual_file_rules: Vec<_> = analysis
        .registry()
        .rules()
        .iter()
        .map(|(name, _)| *name)
        .collect();
    let mut expected_file_rules = file_rules.to_vec();
    actual_file_rules.sort_unstable();
    expected_file_rules.sort_unstable();
    assert_eq!(actual_file_rules, expected_file_rules);
    let mut actual_cross_file_rules: Vec<_> = analysis
        .cross_file_registry()
        .rules()
        .iter()
        .map(|(name, _)| *name)
        .collect();
    let mut expected_cross_file_rules = cross_file_rules.to_vec();
    actual_cross_file_rules.sort_unstable();
    expected_cross_file_rules.sort_unstable();
    assert_eq!(actual_cross_file_rules, expected_cross_file_rules);
    analysis
}

fn assert_flags(analysis: &ProjectAnalysis, expected: (bool, bool, bool)) {
    assert_eq!(
        (
            analysis.resolve(),
            analysis.collect_programs(),
            analysis.cross_file_resolve(),
        ),
        expected,
        "flags are (resolve, collect_programs, cross_file_resolve)"
    );
}

#[test]
fn selected_rule_metadata_distinguishes_each_execution_requirement() {
    for (name, group, cross_file, needs_resolution) in [
        (STRUCTURAL, "suspicious", false, false),
        (SEMANTIC, "suspicious", false, true),
        (CROSS_FILE, "correctness", true, false),
        (RESOLVED_CROSS_FILE, "correctness", true, true),
    ] {
        let metadata = meta_for(name).expect("selected rule exists in metadata");
        assert_eq!(metadata.group.as_str(), group, "{name}");
        let actual = match metadata.kind {
            RuleKind::File {
                needs_resolution, ..
            } => (false, needs_resolution),
            RuleKind::CrossFile {
                needs_resolution, ..
            } => (true, needs_resolution),
        };
        assert_eq!(actual, (cross_file, needs_resolution), "{name}");
    }
}

#[test]
fn no_rules_require_no_project_facts() {
    assert_flags(&plan_for(&[], &[]), (false, false, false));
}

#[test]
fn structural_file_rules_require_no_project_facts() {
    assert_flags(&plan_for(&[STRUCTURAL], &[]), (false, false, false));
}

#[test]
fn semantic_file_rules_resolve_without_collecting_programs() {
    assert_flags(&plan_for(&[SEMANTIC], &[]), (true, false, false));
}

#[test]
fn unresolved_cross_file_rules_collect_without_resolving() {
    assert_flags(&plan_for(&[], &[CROSS_FILE]), (false, true, false));
}

#[test]
fn resolved_cross_file_rules_require_all_project_facts() {
    assert_flags(&plan_for(&[], &[RESOLVED_CROSS_FILE]), (true, true, true));
}

#[test]
fn mixed_rules_combine_independent_requirements() {
    for (file_rules, cross_file_rules, expected) in [
        (&[STRUCTURAL, SEMANTIC][..], &[][..], (true, false, false)),
        (&[STRUCTURAL][..], &[CROSS_FILE][..], (false, true, false)),
        (&[SEMANTIC][..], &[CROSS_FILE][..], (true, true, false)),
        (
            &[STRUCTURAL][..],
            &[RESOLVED_CROSS_FILE][..],
            (true, true, true),
        ),
        (
            &[][..],
            &[CROSS_FILE, RESOLVED_CROSS_FILE][..],
            (true, true, true),
        ),
        (
            &[STRUCTURAL, SEMANTIC][..],
            &[CROSS_FILE, RESOLVED_CROSS_FILE][..],
            (true, true, true),
        ),
    ] {
        assert_flags(&plan_for(file_rules, cross_file_rules), expected);
    }
}

#[test]
fn enabling_autofix_snapshots_is_a_no_op_for_structural_rules() {
    let mut analysis = plan_for(&[STRUCTURAL], &[]);
    analysis.set_autofix_snapshots(true);
    assert_flags(&analysis, (false, false, false));
    analysis.set_autofix_snapshots(false);
    assert_flags(&analysis, (false, false, false));
}

#[test]
fn semantic_autofix_snapshots_enable_collection_and_restore_the_base_plan() {
    let mut analysis = plan_for(&[SEMANTIC], &[]);
    assert_flags(&analysis, (true, false, false));
    analysis.set_autofix_snapshots(true);
    assert_flags(&analysis, (true, true, false));
    analysis.set_autofix_snapshots(true);
    assert_flags(&analysis, (true, true, false));
    analysis.set_autofix_snapshots(false);
    assert_flags(&analysis, (true, false, false));
    analysis.set_autofix_snapshots(false);
    assert_flags(&analysis, (true, false, false));
}

#[test]
fn disabling_autofix_snapshots_cannot_disable_required_cross_file_collection() {
    for (file_rules, cross_file_rules, expected) in [
        (&[][..], &[CROSS_FILE][..], (false, true, false)),
        (&[SEMANTIC][..], &[CROSS_FILE][..], (true, true, false)),
        (&[][..], &[RESOLVED_CROSS_FILE][..], (true, true, true)),
    ] {
        let mut analysis = plan_for(file_rules, cross_file_rules);
        analysis.set_autofix_snapshots(false);
        assert_flags(&analysis, expected);
        analysis.set_autofix_snapshots(true);
        assert_flags(&analysis, expected);
        analysis.set_autofix_snapshots(false);
        assert_flags(&analysis, expected);
    }
}
