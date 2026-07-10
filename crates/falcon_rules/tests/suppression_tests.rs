//! End-to-end suppression: a real rule (`avoid-dynamic`) run through the
//! registry with and without inline `// ignore:` comments.

use falcon_analyze::{AnalyzeContext, RuleRegistry};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_rules::dart_code_linter::avoid_dynamic::AvoidDynamic;
use std::path::Path;

fn run(source: &str) -> Vec<String> {
    let mut registry = RuleRegistry::new();
    registry.register(Box::new(AvoidDynamic));
    let (program, _) = parse(source);
    let config = FalconConfig::default();
    let ctx = AnalyzeContext {
        file_path: Path::new("test.dart"),
        source,
        config: &config,
    };
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
fn same_line_ignore_suppresses() {
    assert!(run("dynamic x = 1; // ignore: avoid-dynamic\n").is_empty());
}

#[test]
fn next_line_ignore_suppresses() {
    assert!(run("// ignore: avoid-dynamic\ndynamic x = 1;\n").is_empty());
}

#[test]
fn ignore_for_file_suppresses() {
    assert!(run("// ignore_for_file: avoid-dynamic\ndynamic x = 1;\n").is_empty());
}

#[test]
fn unrelated_ignore_does_not_suppress() {
    assert_eq!(
        run("dynamic x = 1; // ignore: some-other-rule\n"),
        vec!["avoid-dynamic"]
    );
}

#[test]
fn ignore_inside_string_does_not_suppress() {
    assert_eq!(
        run("var s = '// ignore: avoid-dynamic'; dynamic x = 1;\n"),
        vec!["avoid-dynamic"]
    );
}
