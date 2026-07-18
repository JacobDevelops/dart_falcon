//! End-to-end suppression: a real rule (`avoid-dynamic`) run through the
//! registry with and without inline `// falcon-ignore` comments, plus the
//! malformed-suppression diagnostics reported for bad comments.

use falcon_analyze::{AnalyzeContext, RuleRegistry};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_rules::lint::suspicious::avoid_dynamic::AvoidDynamic;
use falcon_rules::meta::suppression_lookup;
use std::path::Path;

fn run(source: &str) -> Vec<String> {
    let mut registry = RuleRegistry::with_lookup(suppression_lookup);
    registry.register(Box::new(AvoidDynamic));
    let (program, _) = parse(source);
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config);
    registry
        .run_all(&program, &ctx)
        .into_iter()
        .map(|d| d.rule.to_string())
        .collect()
}

#[test]
fn fires_without_suppression() {
    assert_eq!(run("dynamic x = 1;\n"), vec!["avoid-dynamic"]);
}

#[test]
fn same_line_suppresses() {
    assert!(
        run("dynamic x = 1; // falcon-ignore lint/suspicious/avoid-dynamic: legacy\n").is_empty()
    );
}

#[test]
fn next_line_suppresses() {
    assert!(
        run("// falcon-ignore lint/suspicious/avoid-dynamic: legacy\ndynamic x = 1;\n").is_empty()
    );
}

#[test]
fn all_file_suppresses() {
    assert!(
        run("// falcon-ignore-all lint/suspicious/avoid-dynamic: legacy\ndynamic x = 1;\ndynamic y = 2;\n")
            .is_empty()
    );
}

#[test]
fn stacked_comments_suppress_next_line() {
    // avoid-dynamic sits one line above the code (another valid suppression line
    // is between it and the code); stacking must still attach it. Both paths are
    // valid, so no diagnostics at all fire.
    let out = run("// falcon-ignore lint/suspicious/no-equal-arguments: a\n\
         // falcon-ignore lint/suspicious/avoid-dynamic: b\n\
         dynamic x = 1;\n");
    assert!(
        out.is_empty(),
        "stacked comments must suppress avoid-dynamic, got {out:?}"
    );
}

#[test]
fn unrelated_rule_does_not_suppress() {
    assert_eq!(
        run("dynamic x = 1; // falcon-ignore lint/suspicious/no-equal-arguments: y\n"),
        vec!["avoid-dynamic"]
    );
}

#[test]
fn inside_string_does_not_suppress() {
    assert_eq!(
        run("var s = '// falcon-ignore lint/suspicious/avoid-dynamic: y'; dynamic x = 1;\n"),
        vec!["avoid-dynamic"]
    );
}

#[test]
fn dart_ignore_no_longer_suppresses() {
    // Falcon ignores Dart's own `// ignore:` comments now.
    assert_eq!(
        run("dynamic x = 1; // ignore: avoid-dynamic\n"),
        vec!["avoid-dynamic"]
    );
}

#[test]
fn missing_reason_reports_and_does_not_suppress() {
    // No reason → the rule still fires AND a malformed-suppression is emitted.
    let out = run("dynamic x = 1; // falcon-ignore lint/suspicious/avoid-dynamic\n");
    assert!(out.contains(&"avoid-dynamic".to_string()));
    assert!(out.contains(&"malformed-suppression".to_string()));
}

#[test]
fn wrong_group_reports_and_does_not_suppress() {
    let out = run("dynamic x = 1; // falcon-ignore lint/style/avoid-dynamic: y\n");
    assert!(out.contains(&"avoid-dynamic".to_string()));
    assert!(out.contains(&"malformed-suppression".to_string()));
}

#[test]
fn unknown_rule_reports() {
    let out = run("dynamic x = 1; // falcon-ignore lint/suspicious/not-a-rule: y\n");
    assert!(out.contains(&"avoid-dynamic".to_string()));
    assert!(out.contains(&"malformed-suppression".to_string()));
}
