//! Integration tests for the analyze pipeline.

use falcon_cli::{CheckOptions, OutputFormat, run_check};
use std::fs;
use tempfile::tempdir;

/// Test 1: No files found returns zero exit code
#[test]
fn test_run_check_no_files_returns_zero() {
    let temp = tempdir().unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 0);
}

/// Test 2: A violation-free Dart file returns zero
#[test]
fn test_run_check_with_clean_dart_file_returns_zero() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  print('ok');\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 0);
}

/// Test 3: max_errors = Some(0) truncates diagnostics; still zero because no rules
#[test]
fn test_run_check_max_errors_zero() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.dart"), "void main() {}").unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        max_errors: Some(0),
        quiet: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 0);
}

/// Test 4: JSON format doesn't panic
#[test]
fn test_run_check_json_format_no_panic() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  print('ok');\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: false,
        format: OutputFormat::Json,
        ..Default::default()
    });
    assert_eq!(exit_code, 0);
}

/// Test 5: Quiet mode returns exit code without panicking
#[test]
fn test_run_check_quiet_mode() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  print('ok');\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 0);
}

/// Test 6: Nonexistent config path returns error exit code
#[test]
fn test_run_check_with_config_path_nonexistent_returns_error() {
    use std::path::PathBuf;
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.dart"), "void main() {}").unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        config_path: Some(PathBuf::from("/nonexistent/falcon.json")),
        quiet: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 1);
}

/// Test 7: --parallel flag runs analysis and returns zero on violation-free files
#[test]
fn test_run_check_parallel_flag_clean_returns_zero() {
    let temp = tempdir().unwrap();
    // a.dart is the entrypoint and references b.dart's Foo, so the cross-file
    // rules (unused-files / unused-code) stay quiet alongside the per-file rules.
    fs::write(
        temp.path().join("a.dart"),
        "import 'b.dart';\nvoid main() {\n  print(Foo());\n}\n",
    )
    .unwrap();
    fs::write(temp.path().join("b.dart"), "class Foo {}\n").unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        parallel: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 0);
}

/// Test 8: --exit-code flag used as error_exit_code is wired through (no violations → always 0)
#[test]
fn test_run_check_custom_exit_code_no_violations_returns_zero() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  print('ok');\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        error_exit_code: 2,
        ..Default::default()
    });
    // No violations → exit 0 regardless of error_exit_code
    assert_eq!(exit_code, 0);
}

/// Test 9: JSON output snapshot for empty diagnostics
#[test]
fn test_output_json_empty_snapshot() {
    let result = falcon_cli::format_json(&[]);
    insta::assert_snapshot!(result);
}

/// Test 10: Text output snapshot for empty diagnostics
#[test]
fn test_output_text_empty_snapshot() {
    let result = falcon_cli::format_text(&[]);
    insta::assert_snapshot!(result);
}

// ---------------------------------------------------------------------------
// Config-as-Contract: falcon.json must control rule enablement, severity,
// excludes, and max_errors (plan M2.3 / M3.3 / Phase-1 acceptance criteria).
// ---------------------------------------------------------------------------

use falcon_cli::collect_check;
use std::path::PathBuf;

/// Dart source that reliably triggers `avoid-dynamic`.
const DYNAMIC_SRC: &str = "void f() {\n  dynamic x = 1;\n  print(x);\n}\n";

fn options_for(dir: &std::path::Path, config: Option<PathBuf>) -> CheckOptions {
    CheckOptions {
        paths: vec![dir.to_path_buf()],
        config_path: config,
        quiet: true,
        ..Default::default()
    }
}

/// Baseline: with no config, avoid-dynamic fires on DYNAMIC_SRC.
#[test]
fn test_collect_check_default_config_rule_fires() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let out = collect_check(&options_for(temp.path(), None)).unwrap();
    assert!(
        out.diagnostics.iter().any(|d| d.rule == "avoid-dynamic"),
        "expected avoid-dynamic to fire by default"
    );
}

/// Setting a rule to "off" in its group suppresses its diagnostics.
#[test]
fn test_config_disables_rule() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } } }"#,
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert!(
        out.diagnostics.iter().all(|d| d.rule != "avoid-dynamic"),
        "avoid-dynamic must not fire when set to off in config"
    );
}

/// A per-rule level changes the reported severity for a rule.
#[test]
fn test_config_rule_level_severity_applied() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "info" } } } }"#,
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    let diag = out
        .diagnostics
        .iter()
        .find(|d| d.rule == "avoid-dynamic")
        .expect("avoid-dynamic should fire");
    assert_eq!(diag.severity, falcon_diagnostics::Severity::Info);
}

/// files.includes negations from falcon.json are honored (not just CLI --exclude).
#[test]
fn test_config_exclude_via_includes_negation() {
    let temp = tempdir().unwrap();
    let gen_dir = temp.path().join("gen");
    fs::create_dir(&gen_dir).unwrap();
    fs::write(gen_dir.join("a.dart"), DYNAMIC_SRC).unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "files": { "includes": ["**", "!**/gen/**"] } }"#,
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert_eq!(out.total_files, 0, "gen/ files must be excluded via config");
}

/// A domain set to "none" disables that domain's rules while leaving others on.
#[test]
fn test_config_domain_none_disables_flutter_only() {
    const FLUTTER_SRC: &str = "import 'package:flutter/material.dart';\n\
class S extends StatelessWidget {\n\
  Widget _card() {\n\
    dynamic x = 1;\n\
    print(x);\n\
    return Card(child: Text('hi'));\n\
  }\n\
}\n";
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), FLUTTER_SRC).unwrap();

    // Baseline: both a flutter rule and a non-flutter rule fire.
    let base = collect_check(&options_for(temp.path(), None)).unwrap();
    assert!(
        base.diagnostics
            .iter()
            .any(|d| d.rule == "avoid-returning-widgets")
    );
    assert!(base.diagnostics.iter().any(|d| d.rule == "avoid-dynamic"));

    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "domains": { "flutter": "none" } } }"#,
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert!(
        out.diagnostics
            .iter()
            .all(|d| d.rule != "avoid-returning-widgets"),
        "flutter rule must be gated off by domains.flutter=none"
    );
    assert!(
        out.diagnostics.iter().any(|d| d.rule == "avoid-dynamic"),
        "non-flutter rule must remain enabled"
    );
}

/// linter.enabled=false disables every file rule. Project rules are a separate
/// feature, so silencing everything requires disabling `project` too.
#[test]
fn test_config_both_features_disabled_yields_no_diagnostics() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "enabled": false }, "project": { "enabled": false } }"#,
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert!(
        out.diagnostics.is_empty(),
        "no rule should run when both linter and project are disabled"
    );
}

/// max_errors from falcon.json truncates diagnostics; CLI flag wins over config.
#[test]
fn test_config_max_errors_and_cli_precedence() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("a.dart"),
        "void f() {\n  dynamic x = 1;\n  dynamic y = 1;\n  print(x);\n  print(y);\n}\n",
    )
    .unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(&config, r#"{ "max_errors": 1 }"#).unwrap();

    let out = collect_check(&options_for(temp.path(), Some(config.clone()))).unwrap();
    assert_eq!(out.diagnostics.len(), 1, "config max_errors must truncate");

    let mut opts = options_for(temp.path(), Some(config));
    opts.max_errors = Some(2);
    let out = collect_check(&opts).unwrap();
    assert_eq!(
        out.diagnostics.len(),
        2,
        "CLI max_errors must override config"
    );
}

/// Parallel and sequential runs produce identical, deterministically ordered output.
#[test]
fn test_parallel_sequential_output_identical() {
    let temp = tempdir().unwrap();
    for name in ["b.dart", "a.dart", "c.dart"] {
        fs::write(temp.path().join(name), DYNAMIC_SRC).unwrap();
    }
    let seq = collect_check(&options_for(temp.path(), None)).unwrap();
    let mut opts = options_for(temp.path(), None);
    opts.parallel = true;
    let par = collect_check(&opts).unwrap();
    let key = |d: &falcon_diagnostics::Diagnostic| {
        (d.file_path.clone(), d.span.start, d.rule, d.message.clone())
    };
    assert_eq!(
        seq.diagnostics.iter().map(key).collect::<Vec<_>>(),
        par.diagnostics.iter().map(key).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Inline suppression: `// ignore:` / `// ignore_for_file:` honored end to end.
// ---------------------------------------------------------------------------

/// A same-line `// ignore:` suppresses that occurrence; a second unignored
/// occurrence still fires. An `// ignore_for_file:` clears the whole file.
#[test]
fn test_inline_suppression_end_to_end() {
    let temp = tempdir().unwrap();
    // Line 0 is suppressed inline; line 1 is not.
    fs::write(
        temp.path().join("a.dart"),
        "dynamic a = 1; // ignore: avoid-dynamic\ndynamic b = 2;\n",
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), None)).unwrap();
    let dynamic_hits = out
        .diagnostics
        .iter()
        .filter(|d| d.rule == "avoid-dynamic")
        .count();
    assert_eq!(dynamic_hits, 1, "only the unignored occurrence should fire");

    // ignore_for_file clears every occurrence in the file.
    fs::write(
        temp.path().join("a.dart"),
        "// ignore_for_file: avoid-dynamic\ndynamic a = 1;\ndynamic b = 2;\n",
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), None)).unwrap();
    assert!(
        out.diagnostics.iter().all(|d| d.rule != "avoid-dynamic"),
        "ignore_for_file must suppress every avoid-dynamic in the file"
    );
}

/// An override turning a firing rule off under `gen/**` suppresses it only for
/// matching files — other paths still report it.
#[test]
fn test_override_disables_rule_for_matching_path_only() {
    let temp = tempdir().unwrap();
    let gen_dir = temp.path().join("gen");
    fs::create_dir(&gen_dir).unwrap();
    fs::write(gen_dir.join("drop.dart"), DYNAMIC_SRC).unwrap();
    fs::write(temp.path().join("keep.dart"), DYNAMIC_SRC).unwrap();

    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{
            "overrides": [ {
                "includes": ["**/gen/**"],
                "linter": { "rules": { "suspicious": { "avoid-dynamic": "off" } } }
            } ]
        }"#,
    )
    .unwrap();

    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.rule == "avoid-dynamic" && d.file_path.ends_with("keep.dart")),
        "avoid-dynamic must still fire outside the override path"
    );
    assert!(
        out.diagnostics
            .iter()
            .all(|d| !(d.rule == "avoid-dynamic" && d.file_path.contains("gen"))),
        "avoid-dynamic must be suppressed under gen/ by the override"
    );
}
