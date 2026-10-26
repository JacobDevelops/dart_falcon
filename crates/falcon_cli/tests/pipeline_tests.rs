//! Integration tests for the analyze pipeline.

use falcon_cli::{CheckOptions, CheckOutput, OutputFormat};
use falcon_lsp::LspState;
use std::fs;
use tempfile::tempdir;

fn collect_check(options: &CheckOptions) -> Result<CheckOutput, String> {
    falcon_cli::collect_check_with_sdk_locator(options, |_| None)
}

fn run_check(options: CheckOptions) -> i32 {
    falcon_cli::run_check_with_sdk_locator(options, |_| None)
}

#[test]
fn oversized_sources_abort_before_partial_workspace_analysis() {
    for name in ["oversized.dart", "pubspec.yaml"] {
        let temp = tempdir().unwrap();
        let config = temp.path().join("falcon.json");
        fs::write(
            &config,
            r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
        )
        .unwrap();
        fs::write(temp.path().join("orphan.dart"), "class Orphan {}").unwrap();
        fs::File::create(temp.path().join(name))
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
        let options = CheckOptions {
            paths: vec![temp.path().to_path_buf()],
            config_path: Some(config),
            quiet: true,
            ..Default::default()
        };
        let error = collect_check(&options).unwrap_err();
        assert!(error.contains(name));
        assert!(error.contains("maximum supported source size"));
        assert_ne!(run_check(options), 0);
    }
}

#[test]
fn secure_pubspec_urls_uses_security_configuration_and_rejects_old_group() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("pubspec.yaml"),
        "name: insecure_package\nhomepage: http://example.com\n",
    )
    .unwrap();
    let config_path = temp.path().join("falcon.json");
    for (group, level, expected_severity) in [
        (
            "security",
            "error",
            Some(falcon_diagnostics::Severity::Error),
        ),
        ("security", "off", None),
        ("suspicious", "error", None),
    ] {
        let config = serde_json::json!({
            "linter": { "rules": { "recommended": false } },
            "cross-file": {
                "rules": {
                    "recommended": false,
                    (group): { "secure-pubspec-urls": level }
                }
            }
        });
        fs::write(&config_path, config.to_string()).unwrap();
        if group == "suspicious" {
            let error =
                collect_check(&options_for(temp.path(), Some(config_path.clone()))).unwrap_err();
            assert!(
                error.contains("cross-file.rules.suspicious.secure-pubspec-urls"),
                "{error}"
            );
            continue;
        }
        let output = collect_check(&options_for(temp.path(), Some(config_path.clone()))).unwrap();
        match expected_severity {
            Some(severity) => {
                assert_eq!(output.diagnostics.len(), 1, "{group}: {level}");
                let diagnostic = &output.diagnostics[0];
                assert_eq!(diagnostic.rule, "secure-pubspec-urls");
                assert_eq!(diagnostic.severity, severity);
                assert!(diagnostic.file_path.ends_with("pubspec.yaml"));
                assert_eq!(output.exit_code, 1);
            }
            None => {
                assert!(output.diagnostics.is_empty(), "{group}: {level}");
                assert_eq!(output.exit_code, 0);
            }
        }
    }
}

#[test]
fn manifest_discovery_does_not_turn_non_dart_inputs_into_processed_files() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("pubspec.yaml"), "name: example\n").unwrap();
    fs::write(temp.path().join("README.txt"), "package notes").unwrap();
    let options = options_for(&temp.path().join("README.txt"), None);
    let output = collect_check(&options).unwrap();
    assert_eq!(output.exit_code, 1);
    assert!(
        output
            .diagnostics
            .iter()
            .any(|d| d.rule == "unmatched-path")
    );
    assert_eq!(output.total_files, 0);
    let output = collect_check(&CheckOptions {
        no_errors_on_unmatched: true,
        ..options
    })
    .unwrap();
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.rule != "unmatched-path")
    );
}

#[test]
fn manifest_only_inputs_are_processed_without_unmatched_diagnostics() {
    let temp = tempdir().unwrap();
    let manifest = temp.path().join("pubspec.yaml");
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,"security":{"secure-pubspec-urls":"error"}}}}"#,
    )
    .unwrap();
    for (source, expected_findings) in [
        ("name: example\n", 0),
        ("name: example\nhomepage: http://example.com\n", 1),
    ] {
        fs::write(&manifest, source).unwrap();
        for path in [temp.path(), manifest.as_path()] {
            let output = collect_check(&options_for(path, Some(config.clone()))).unwrap();
            assert_eq!(output.exit_code, i32::from(expected_findings != 0));
            assert_eq!(output.diagnostics.len(), expected_findings);
            assert!(
                output
                    .diagnostics
                    .iter()
                    .all(|d| d.rule == "secure-pubspec-urls")
            );
        }
    }
}

#[test]
fn filtered_explicit_inputs_explain_why_they_are_unmatched() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let config = root.join("falcon.json");
    fs::write(
        &config,
        r#"{"files":{"includes":["lib/**/*.dart"]},"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let valid = root.join("lib/main.dart");
    fs::create_dir(root.join("lib")).unwrap();
    fs::write(&valid, "void main() {}").unwrap();
    for path in [root.join("other.dart"), temp.path().join("outside.dart")] {
        fs::write(&path, "void main() {}").unwrap();
        for with_valid in [false, true] {
            let mut options = options_for(&path, Some(config.clone()));
            options.paths.push(path.clone());
            if with_valid {
                options.paths.push(valid.clone());
            }
            let output = collect_check(&options).unwrap();
            assert_eq!(output.exit_code, 1);
            assert_eq!(output.diagnostics.len(), 1);
            let diagnostic = &output.diagnostics[0];
            assert_eq!(diagnostic.rule, "unmatched-path");
            assert_eq!(diagnostic.file_path, path.to_string_lossy());
            assert!(
                diagnostic.message.contains("excluded by files.includes"),
                "{}",
                diagnostic.message
            );
            assert_eq!((diagnostic.line, diagnostic.col), (0, 0));
            options.no_errors_on_unmatched = true;
            let output = collect_check(&options).unwrap();
            assert_eq!(output.exit_code, 0);
            assert!(output.diagnostics.is_empty());
        }
    }
}

#[test]
fn manifest_only_inputs_are_processed_with_cross_file_rules_disabled() {
    let temp = tempdir().unwrap();
    let manifest = temp.path().join("pubspec.yaml");
    fs::write(&manifest, "name: example\n").unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    for path in [temp.path(), manifest.as_path()] {
        let output = collect_check(&options_for(path, Some(config.clone()))).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.diagnostics.is_empty());
    }
}

#[test]
fn empty_directories_and_unsupported_files_keep_generic_unmatched_reason() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{"files":{"includes":["lib/**/*.dart"]},"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let empty = temp.path().join("lib");
    fs::create_dir(&empty).unwrap();
    let readme = temp.path().join("README.txt");
    fs::write(&readme, "package notes").unwrap();
    let outside = tempdir().unwrap();
    for path in [empty.as_path(), readme.as_path(), outside.path()] {
        let output = collect_check(&options_for(path, Some(config.clone()))).unwrap();
        assert_eq!(output.exit_code, 1);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].message, "No files were processed.");
    }
}

#[test]
fn ancestor_manifests_do_not_make_empty_or_filtered_directories_processed() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("pubspec.yaml"), "name: example\n").unwrap();
    let empty = temp.path().join("lib");
    let filtered = temp.path().join("test");
    fs::create_dir(&empty).unwrap();
    fs::create_dir(&filtered).unwrap();
    fs::write(filtered.join("example.dart"), "void main() {}").unwrap();
    let config = temp.path().join("falcon.json");
    for enabled in [false, true] {
        fs::write(
            &config,
            serde_json::json!({
                "files": {"includes": ["lib/**/*.dart"]},
                "linter": {"enabled": false},
                "cross-file": {
                    "enabled": enabled,
                    "rules": {
                        "recommended": false,
                        "security": {"secure-pubspec-urls": "error"}
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        for path in [&empty, &filtered] {
            let mut options = options_for(path, Some(config.clone()));
            let output = collect_check(&options).unwrap();
            assert_eq!(output.exit_code, 1);
            assert_eq!(output.total_files, 0);
            assert_eq!(output.diagnostics.len(), 1);
            let diagnostic = &output.diagnostics[0];
            assert_eq!(diagnostic.rule, "unmatched-path");
            assert_eq!(diagnostic.message, "No files were processed.");
            assert_eq!(diagnostic.file_path, path.to_string_lossy());
            assert_eq!((diagnostic.line, diagnostic.col), (0, 0));
            options.no_errors_on_unmatched = true;
            let output = collect_check(&options).unwrap();
            assert_eq!(output.exit_code, 0);
            assert!(output.diagnostics.is_empty());
        }
    }
}

#[test]
fn excluded_manifests_cannot_report_cross_file_diagnostics_from_disk() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("main.dart"), "void main() {}").unwrap();
    fs::write(
        temp.path().join("pubspec.yaml"),
        "name: example\nhomepage: http://example.com\n",
    )
    .unwrap();
    let config = temp.path().join("falcon.json");
    for cli_exclude in [false, true] {
        let mut settings = serde_json::json!({
            "linter": {"rules": {"recommended": false}},
            "cross-file": {"rules": {
                "recommended": false,
                "security": {"secure-pubspec-urls": "error"}
            }}
        });
        if !cli_exclude {
            settings["files"] = serde_json::json!({"includes": ["**", "!pubspec.yaml"]});
        }
        fs::write(&config, settings.to_string()).unwrap();
        let options = CheckOptions {
            exclude_patterns: if cli_exclude {
                vec!["pubspec.yaml".to_owned()]
            } else {
                Vec::new()
            },
            ..options_for(temp.path(), Some(config.clone()))
        };
        let output = collect_check(&options).unwrap();
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.exit_code, 0);
    }
}

#[test]
fn unmatched_inputs_are_error_diagnostics_unless_opted_out() {
    let temp = tempdir().unwrap();
    for path in [temp.path().to_path_buf(), temp.path().join("missing.dart")] {
        let mut options = options_for(&path, None);
        options.error_exit_code = 0;
        let output = collect_check(&options).unwrap();
        assert_eq!(output.exit_code, 1);
        assert_eq!(output.total_files, 0);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].rule, "unmatched-path");
        assert_eq!(output.diagnostics[0].severity, Severity::Error);

        options.no_errors_on_unmatched = true;
        let output = collect_check(&options).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.diagnostics.is_empty());
    }
}

#[test]
fn non_directory_input_components_are_unmatched_even_with_a_valid_input() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let source = temp.path().join("main.dart");
    fs::write(&source, "void main() {}").unwrap();
    for invalid in [
        source.join("child.dart"),
        PathBuf::from(format!("{}/", source.display())),
    ] {
        let mut options = CheckOptions {
            paths: vec![source.clone(), invalid.clone()],
            error_exit_code: 0,
            ..options_for(temp.path(), Some(config.clone()))
        };
        let output = collect_check(&options).unwrap();
        assert_eq!(output.total_files, 1);
        assert_eq!(output.exit_code, 1);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].rule, "unmatched-path");
        assert_eq!(output.diagnostics[0].file_path, invalid.to_string_lossy());
        options.no_errors_on_unmatched = true;
        let output = collect_check(&options).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.diagnostics.is_empty());
    }
}

#[test]
fn no_paths_defaults_to_the_current_directory() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{"linter":{"enabled":false},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    let output = collect_check(&CheckOptions {
        config_path: Some(config),
        exclude_patterns: vec![
            std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(output.exit_code, 1);
    assert_eq!(output.total_files, 0);
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].file_path, ".");
    assert_eq!(output.diagnostics[0].message, "No files were processed.");
}

#[test]
fn missing_input_does_not_hide_valid_file_diagnostics() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(&config, r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false}}}"#).unwrap();
    let source = temp.path().join("broken.dart");
    fs::write(&source, "void main( {").unwrap();
    let output = collect_check(&CheckOptions {
        paths: vec![source, temp.path().join("missing.dart")],
        config_path: Some(config),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(output.total_files, 1);
    assert_eq!(output.exit_code, 1);
    assert!(
        output
            .diagnostics
            .iter()
            .any(|d| d.rule == "unmatched-path")
    );
    assert!(output.diagnostics.iter().any(|d| d.rule == "syntax-error"));
}

#[test]
fn max_errors_limits_printing_not_collected_diagnostics_or_status() {
    let temp = tempdir().unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(&config, r#"{"max-errors":1,"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false}}}"#).unwrap();
    fs::write(temp.path().join("broken.dart"), "void main( {").unwrap();
    fs::write(temp.path().join("also_broken.dart"), "void other( {").unwrap();
    for max_errors in [None, Some(0), Some(1)] {
        let options = CheckOptions {
            max_errors,
            error_exit_code: 7,
            ..options_for(temp.path(), Some(config.clone()))
        };
        let output = collect_check(&options).unwrap();
        assert!(output.diagnostics.len() >= 2);
        assert_eq!(output.exit_code, 7);
        assert_eq!(run_check(options), 7);
    }
}

/// Test 2: A violation-free Dart file returns zero
#[test]
fn test_run_check_with_clean_dart_file_returns_zero() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  const greeting = 'hello';\n  assert(greeting.isNotEmpty);\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        ..options_for(temp.path(), None)
    });
    assert_eq!(exit_code, 0);
}

/// A clean file still succeeds with a zero print limit.
#[test]
fn test_run_check_max_errors_zero() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  const greeting = 'hello';\n  assert(greeting.isNotEmpty);\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        max_errors: Some(0),
        quiet: true,
        ..options_for(temp.path(), None)
    });
    assert_eq!(exit_code, 0);
}

/// Test 4: JSON format doesn't panic
#[test]
fn test_run_check_json_format_no_panic() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  const greeting = 'hello';\n  assert(greeting.isNotEmpty);\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: false,
        format: OutputFormat::Json,
        ..options_for(temp.path(), None)
    });
    assert_eq!(exit_code, 0);
}

/// Test 5: Quiet mode returns exit code without panicking
#[test]
fn test_run_check_quiet_mode() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  const greeting = 'hello';\n  assert(greeting.isNotEmpty);\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        ..options_for(temp.path(), None)
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
        error_exit_code: 0,
        max_errors: Some(0),
        ..Default::default()
    });
    assert_eq!(exit_code, 1);
}

/// Test 7: Default analysis returns zero on violation-free files across imports.
#[test]
fn test_run_check_default_clean_imports_returns_zero() {
    let temp = tempdir().unwrap();
    // a.dart is the entrypoint and references foo.dart's Foo, so the cross-file
    // rules (unused-files / unused-code) stay quiet alongside the per-file rules.
    fs::write(
        temp.path().join("a.dart"),
        "import 'foo.dart';\nvoid main() {\n  final foo = Foo();\n  assert(foo.hashCode >= 0);\n}\n",
    )
    .unwrap();
    fs::write(temp.path().join("foo.dart"), "class Foo {}\n").unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        ..options_for(temp.path(), None)
    });
    assert_eq!(exit_code, 0);
}

/// Test 8: --exit-code flag used as error_exit_code is wired through (no violations → always 0)
#[test]
fn test_run_check_custom_exit_code_no_violations_returns_zero() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("test.dart"),
        "void main() {\n  const greeting = 'hello';\n  assert(greeting.isNotEmpty);\n}\n",
    )
    .unwrap();
    let exit_code = run_check(CheckOptions {
        paths: vec![temp.path().to_path_buf()],
        quiet: true,
        error_exit_code: 2,
        ..options_for(temp.path(), None)
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
// Parse errors must surface as `syntax-error` diagnostics (severity error),
// listed ahead of that file's lints, and count toward the error exit code.
// ---------------------------------------------------------------------------

use falcon_diagnostics::Severity;

/// Non-compiling Dart (missing close paren) yields a `syntax-error` diagnostic
/// at the recovered offset with severity Error, and drives a nonzero exit code.
#[test]
fn test_parse_error_surfaces_as_syntax_error() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("typo.dart"),
        "void main() {\n  print(\"hello\"\n}\n",
    )
    .unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(&config, "{}").unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();

    let syntax: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.rule == "syntax-error")
        .collect();
    assert_eq!(syntax.len(), 1, "exactly one syntax-error expected");
    assert_eq!(syntax[0].severity, Severity::Error);
    // Resolved 1-based position: the '}' recovery point on line 3, col 1.
    assert_eq!((syntax[0].line, syntax[0].col), (3, 1));
    assert_eq!(out.exit_code, 1, "syntax errors must fail the run");
}

/// Lints on the recovered syntax tree still fire, but the syntax error is listed first
/// for the file.
#[test]
fn test_syntax_error_listed_before_lints() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("typo.dart"),
        "void main() {\n  dynamic value = 1;\n  print(\"hello\"\n}\n",
    )
    .unwrap();
    let out = collect_check(&options_for_dynamic(temp.path())).unwrap();

    assert!(
        out.diagnostics.iter().any(|d| d.rule == "avoid-dynamic"),
        "lints on the recovered syntax tree still fire"
    );
    let first_for_file = out
        .diagnostics
        .iter()
        .find(|d| d.file_path.ends_with("typo.dart"))
        .expect("a diagnostic for typo.dart");
    assert_eq!(
        first_for_file.rule, "syntax-error",
        "parse errors must be listed first for the file"
    );
}

// Config controls rule enablement, severity, excludes, and max_errors.

use std::path::PathBuf;

/// Dart source that reliably triggers `avoid-dynamic`.
const DYNAMIC_SRC: &str = "void f() {\n  dynamic x = 1;\n  print(x);\n}\n";

fn options_for(dir: &std::path::Path, config: Option<PathBuf>) -> CheckOptions {
    let config = config.or_else(|| {
        let base = if dir.is_dir() {
            dir
        } else {
            dir.parent().unwrap()
        };
        let path = base.join("falcon.json");
        fs::write(&path, "{}").unwrap();
        Some(path)
    });
    CheckOptions {
        paths: vec![dir.to_path_buf()],
        config_path: config,
        quiet: true,
        ..Default::default()
    }
}

fn options_for_dynamic(dir: &std::path::Path) -> CheckOptions {
    let config = dir.join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "rules": { "suspicious": { "avoid-dynamic": "warn" } } } }"#,
    )
    .unwrap();
    options_for(dir, Some(config))
}

/// An empty config enables recommended rules without inheriting repository settings.
#[test]
fn test_collect_check_default_config_rule_fires() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(&config, "{}").unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.rule == "prefer-final-locals"),
        "expected recommended prefer-final-locals to fire by default"
    );
    assert!(
        out.diagnostics.iter().all(|d| d.rule != "avoid-dynamic"),
        "opt-in avoid-dynamic must not fire by default"
    );
}

/// Setting a rule to "off" in its group suppresses its diagnostics.
#[test]
fn test_config_disables_rule() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let baseline = collect_check(&options_for_dynamic(temp.path())).unwrap();
    assert!(
        baseline
            .diagnostics
            .iter()
            .any(|d| d.rule == "avoid-dynamic")
    );
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

    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "domains": { "flutter": "all" }, "rules": { "recommended": false, "suspicious": { "avoid-dynamic": "warn" } } } }"#,
    )
    .unwrap();
    let base = collect_check(&options_for(temp.path(), Some(config.clone()))).unwrap();
    assert!(
        base.diagnostics
            .iter()
            .any(|d| d.rule == "avoid-returning-widgets")
    );
    assert!(base.diagnostics.iter().any(|d| d.rule == "avoid-dynamic"));

    fs::write(
        &config,
        r#"{ "linter": { "domains": { "flutter": "none" }, "rules": { "recommended": false, "suspicious": { "avoid-dynamic": "warn" } } } }"#,
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

/// linter.enabled=false disables every file rule. Cross-file rules are a separate
/// feature, so silencing everything requires disabling `cross_file` too.
#[test]
fn test_config_both_features_disabled_yields_no_diagnostics() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), DYNAMIC_SRC).unwrap();
    let config = temp.path().join("falcon.json");
    fs::write(
        &config,
        r#"{ "linter": { "enabled": false }, "cross-file": { "enabled": false } }"#,
    )
    .unwrap();
    let out = collect_check(&options_for(temp.path(), Some(config))).unwrap();
    assert!(
        out.diagnostics.is_empty(),
        "no rule should run when both linter and cross_file are disabled"
    );
}

#[test]
fn unnecessary_nullable_is_independent_of_enabled_file_rules() {
    let temp = tempdir().unwrap();
    let app = temp.path().join("app");
    let dependency = temp.path().join("dep/lib");
    fs::create_dir_all(app.join(".dart_tool")).unwrap();
    fs::create_dir_all(&dependency).unwrap();
    fs::write(dependency.join("api.dart"), "int value() => 1;").unwrap();
    fs::write(
        app.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[
            {"name":"app","rootUri":"../","packageUri":""},
            {"name":"dep","rootUri":"../../dep","packageUri":"lib/"}
        ]}"#,
    )
    .unwrap();
    fs::write(
        app.join("a.dart"),
        "import 'package:dep/api.dart'; void _take(int? value) {} void use() { _take(value()); }",
    )
    .unwrap();
    let config = app.join("falcon.json");
    let mut results = Vec::with_capacity(2);
    for enabled in [false, true] {
        fs::write(&config, serde_json::json!({
            "linter": {
                "enabled": enabled,
                "rules": { "recommended": false, "suspicious": { "avoid-ignoring-return-values": "warn" } }
            },
            "cross-file": {
                "rules": { "recommended": false, "correctness": { "unnecessary-nullable": "warn" } }
            }
        }).to_string()).unwrap();
        let loaded = falcon_rules::load_config(&config).unwrap().config;
        assert_eq!(
            falcon_rules::requires_resolution(&falcon_rules::build_registry(&loaded)),
            enabled
        );
        assert!(falcon_rules::cross_file_requires_resolution(&loaded));
        let out = collect_check(&options_for(&app, Some(config.clone()))).unwrap();
        results.push(
            out.diagnostics
                .into_iter()
                .filter(|diagnostic| diagnostic.rule == "unnecessary-nullable")
                .map(|diagnostic| (diagnostic.message, diagnostic.span))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(results[0].len(), 1);
    assert_eq!(results[0], results[1]);
}

/// max_errors controls output only; the CLI flag wins over the config value.
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
    assert!(out.diagnostics.len() > 2);
    assert_eq!(out.max_errors, Some(1));
    let diagnostic_count = out.diagnostics.len();

    let mut opts = options_for(temp.path(), Some(config));
    opts.max_errors = Some(2);
    let out = collect_check(&opts).unwrap();
    assert_eq!(out.diagnostics.len(), diagnostic_count);
    assert_eq!(
        out.max_errors,
        Some(2),
        "CLI max_errors must override config"
    );
}

/// Default CLI analysis matches the sequential library oracle in stable order.
#[test]
fn test_default_output_matches_ordered_sequential_diagnostics() {
    let temp = tempdir().unwrap();
    fs::create_dir_all(temp.path().join("lib/src")).unwrap();
    let files = [
        ("b.dart", "void f(bool flag) {\n  dynamic x = 1;\n  dynamic y = 2;\n  if (flag == true) { print(x); }\n  print(y);\n}\n"),
        ("a.dart", DYNAMIC_SRC),
        ("lib/src/orphan.dart", DYNAMIC_SRC),
        ("typo.dart", "void main() {\n  dynamic value = 1;\n  print(\"hello\"\n}\n"),
    ]
    .map(|(name, source)| {
        let path = temp.path().join(name);
        fs::write(&path, source).unwrap();
        (path, source.to_owned())
    });
    let config_path = temp.path().join("falcon.json");
    fs::write(
        &config_path,
        r#"{ "linter": { "rules": { "recommended": false, "suspicious": { "avoid-dynamic": "warn" }, "complexity": { "no-boolean-literal-compare": "warn" } } }, "cross-file": { "rules": { "recommended": false, "correctness": { "unused-files": "warn" } } } }"#,
    )
    .unwrap();
    let config = falcon_rules::load_config(&config_path).unwrap().config;
    let registry = falcon_rules::build_registry(&config);
    assert!(falcon_rules::requires_resolution(&registry));
    let cross_file_registry = falcon_rules::build_cross_file_registry(&config);
    assert!(!cross_file_registry.is_empty());
    let packages = falcon_analyze::package_identities(files.iter().map(|(path, _)| path.as_path()));
    let environment = falcon_analyze::AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(|(path, _)| path.as_path()),
        |_| None,
    );
    let (mut sequential, project_files, indexes) =
        falcon_analyze::analyze_sequential_collecting_resolving(
            &registry,
            &files,
            &config,
            true,
            true,
            falcon_analyze::Workspace {
                packages: &packages,
                environment: Some(&environment),
            },
        );
    falcon_rules::apply_severities(&mut sequential, &config);
    sequential.extend(
        falcon_rules::run_cross_file(
            &environment,
            &cross_file_registry,
            &project_files,
            &config,
            indexes.as_ref(),
        )
        .unwrap(),
    );
    for diagnostic in &mut sequential {
        let (_, source) = files
            .iter()
            .find(|(path, _)| path.to_string_lossy() == diagnostic.file_path)
            .unwrap();
        diagnostic.resolve_position(source);
    }
    sequential.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then((a.rule != "syntax-error").cmp(&(b.rule != "syntax-error")))
            .then(a.span.start.cmp(&b.span.start))
            .then(a.rule.cmp(b.rule))
            .then(a.span.end.cmp(&b.span.end))
            .then(a.message.cmp(&b.message))
    });
    for rule in [
        "avoid-dynamic",
        "no-boolean-literal-compare",
        "unused-files",
        "syntax-error",
    ] {
        assert!(
            sequential.iter().any(|diagnostic| diagnostic.rule == rule),
            "missing oracle diagnostic for {rule}"
        );
    }

    let mut options = options_for(temp.path(), Some(config_path));
    let output = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert_eq!(output.total_files, files.len());
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        serde_json::to_value(&output.diagnostics).unwrap(),
        serde_json::to_value(&sequential).unwrap()
    );

    options.max_errors = Some(2);
    let limited = falcon_cli::collect_check_with_sdk_locator(&options, |_| None).unwrap();
    assert_eq!(
        serde_json::to_value(&limited.diagnostics).unwrap(),
        serde_json::to_value(&sequential).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Inline suppression: `// falcon-ignore` / `// falcon-ignore-all` end to end.
// ---------------------------------------------------------------------------

/// A same-line `// falcon-ignore` suppresses that occurrence; a second
/// unsuppressed occurrence still fires. `// falcon-ignore-all` clears the file.
#[test]
fn test_inline_suppression_end_to_end() {
    let temp = tempdir().unwrap();
    // Line 0 is suppressed inline; line 1 is not.
    fs::write(
        temp.path().join("a.dart"),
        "dynamic a = 1; // falcon-ignore lint/suspicious/avoid-dynamic: legacy\ndynamic b = 2;\n",
    )
    .unwrap();
    let options = options_for_dynamic(temp.path());
    let out = collect_check(&options).unwrap();
    let dynamic_hits = out
        .diagnostics
        .iter()
        .filter(|d| d.rule == "avoid-dynamic")
        .count();
    assert_eq!(
        dynamic_hits, 1,
        "only the unsuppressed occurrence should fire"
    );

    // falcon-ignore-all clears every occurrence in the file.
    fs::write(
        temp.path().join("a.dart"),
        "// falcon-ignore-all lint/suspicious/avoid-dynamic: legacy\ndynamic a = 1;\ndynamic b = 2;\n",
    )
    .unwrap();
    let out = collect_check(&options).unwrap();
    assert!(
        out.diagnostics.iter().all(|d| d.rule != "avoid-dynamic"),
        "falcon-ignore-all must suppress every avoid-dynamic in the file"
    );
}

/// A `// falcon-ignore` with no reason does not suppress and surfaces a
/// `malformed-suppression` warning; an old Dart `// ignore:` is inert.
#[test]
fn test_malformed_suppression_reported() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("a.dart"),
        "dynamic a = 1; // falcon-ignore lint/suspicious/avoid-dynamic\n\
         dynamic b = 2; // ignore: avoid-dynamic\n",
    )
    .unwrap();
    let out = collect_check(&options_for_dynamic(temp.path())).unwrap();
    // Both occurrences still fire (no-reason comment and the inert Dart comment).
    assert_eq!(
        out.diagnostics
            .iter()
            .filter(|d| d.rule == "avoid-dynamic")
            .count(),
        2,
        "neither a reasonless falcon-ignore nor a Dart // ignore: suppresses"
    );
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.rule == "malformed-suppression"),
        "the reasonless falcon-ignore must report malformed-suppression"
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
            "linter": { "rules": { "recommended": false, "suspicious": { "avoid-dynamic": "warn" } } },
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

fn write_identity_config(root: &std::path::Path) -> PathBuf {
    let config = root.join("falcon.json");
    fs::write(
        &config,
        r#"{
          "linter": { "rules": { "suspicious": {
            "avoid-types-as-parameter-names": "warn"
          } } }
        }"#,
    )
    .unwrap();
    config
}

fn identity_diagnostic_count(options: CheckOptions) -> usize {
    collect_check(&options)
        .unwrap()
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "avoid-types-as-parameter-names")
        .count()
}

#[test]
fn package_identity_is_discovered_for_direct_files_with_yaml_name_syntax() {
    let temp = tempdir().unwrap();
    let lib = temp.path().join("lib");
    fs::create_dir(&lib).unwrap();
    fs::write(
        temp.path().join("pubspec.yaml"),
        "# name: ignored\nname: 'direct_pkg' # quoted YAML value\n",
    )
    .unwrap();
    let types = lib.join("types.dart");
    let main = lib.join("main.dart");
    fs::write(&types, "class PackageType {}\n").unwrap();
    fs::write(
        &main,
        "import 'package:direct_pkg/types.dart';\nvoid f(PackageType) {}\n",
    )
    .unwrap();

    assert_eq!(
        identity_diagnostic_count(CheckOptions {
            paths: vec![main, types],
            config_path: Some(write_identity_config(temp.path())),
            quiet: true,
            ..Default::default()
        }),
        1
    );
}

#[test]
fn package_identity_is_discovered_for_lib_directory_invocation() {
    let temp = tempdir().unwrap();
    let lib = temp.path().join("lib");
    fs::create_dir(&lib).unwrap();
    fs::write(temp.path().join("pubspec.yaml"), "name: \"lib_pkg\"\n").unwrap();
    fs::write(lib.join("types.dart"), "class PackageType {}\n").unwrap();
    fs::write(
        lib.join("main.dart"),
        "import 'package:lib_pkg/types.dart';\nvoid f(PackageType) {}\n",
    )
    .unwrap();

    assert_eq!(
        identity_diagnostic_count(CheckOptions {
            paths: vec![lib],
            config_path: Some(write_identity_config(temp.path())),
            quiet: true,
            ..Default::default()
        }),
        1
    );
}

#[test]
fn package_identity_supports_nested_multi_package_analysis() {
    let temp = tempdir().unwrap();
    for package in ["alpha", "beta"] {
        let root = temp.path().join("packages").join(package);
        let lib = root.join("lib");
        fs::create_dir_all(&lib).unwrap();
        fs::write(root.join("pubspec.yaml"), format!("name: {package}\n")).unwrap();
        fs::write(lib.join("types.dart"), "class PackageType {}\n").unwrap();
        fs::write(
            lib.join("main.dart"),
            format!("import 'package:{package}/types.dart';\nvoid f(PackageType) {{}}\n"),
        )
        .unwrap();
    }

    assert_eq!(
        identity_diagnostic_count(CheckOptions {
            paths: vec![temp.path().join("packages")],
            config_path: Some(write_identity_config(temp.path())),
            quiet: true,
            ..Default::default()
        }),
        2
    );
}

#[test]
fn sibling_workspace_package_name_binds_analyzed_project_file() {
    let temp = tempdir().unwrap();
    let outer_lib = temp.path().join("lib");
    let inner = temp.path().join("tools/inner");
    let inner_lib = inner.join("lib");
    fs::create_dir_all(&outer_lib).unwrap();
    fs::create_dir_all(&inner_lib).unwrap();
    fs::write(temp.path().join("pubspec.yaml"), "name: outer\n").unwrap();
    fs::write(inner.join("pubspec.yaml"), "name: inner\n").unwrap();
    fs::write(outer_lib.join("foreign.dart"), "class ForeignType {}\n").unwrap();
    fs::write(inner_lib.join("own.dart"), "class InnerType {}\n").unwrap();
    fs::write(
        inner_lib.join("main.dart"),
        "import 'package:inner/own.dart';\nimport 'package:outer/foreign.dart';\nvoid f(InnerType, ForeignType) {}\n",
    )
    .unwrap();

    assert_eq!(
        identity_diagnostic_count(CheckOptions {
            paths: vec![temp.path().to_path_buf()],
            config_path: Some(write_identity_config(temp.path())),
            quiet: true,
            ..Default::default()
        }),
        2
    );
}

#[test]
fn resolver_backed_rule_has_cli_lsp_parity() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("falcon.json");
    fs::write(
        &config_path,
        r#"{
          "linter": { "rules": { "correctness": {
            "use-key-in-widget-constructors": "warn"
          } } }
        }"#,
    )
    .unwrap();
    let dart_path = temp.path().join("widget.dart");
    let source =
        "import 'package:flutter/widgets.dart';\nclass Missing extends StatelessWidget {}\n";
    fs::write(&dart_path, source).unwrap();

    let cli = collect_check(&CheckOptions {
        paths: vec![dart_path.clone()],
        config_path: Some(config_path.clone()),
        quiet: true,
        ..Default::default()
    })
    .unwrap();
    let mut lsp = LspState::new_with_sdk_locator(Some(config_path), |_| None);
    let uri = format!("file://{}", dart_path.display());
    let lsp = lsp.open(&uri, source.to_string(), Some(1));

    let cli_count = cli
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "use-key-in-widget-constructors")
        .count();
    let lsp_count = lsp
        .iter()
        .filter(|diagnostic| diagnostic.rule == "use-key-in-widget-constructors")
        .count();
    assert_eq!(cli_count, 1);
    assert_eq!(lsp_count, cli_count);
}

#[test]
fn package_discovery_retains_invalid_and_unreadable_nested_manifest_boundaries() {
    let temp = tempdir().unwrap();
    let outer_lib = temp.path().join("lib");
    let nested = temp.path().join("tools/nested");
    let nested_lib = nested.join("lib");
    fs::create_dir_all(&outer_lib).unwrap();
    fs::create_dir_all(&nested_lib).unwrap();
    fs::write(temp.path().join("pubspec.yaml"), "name: outer\n").unwrap();
    let outer_file = outer_lib.join("outer.dart");
    let nested_file = nested_lib.join("nested.dart");
    let outer_root = falcon_analyze::canonical_or_lexical(&outer_lib);
    let nested_root = falcon_analyze::canonical_or_lexical(&nested_lib);
    for manifest in [b"name: [\n".as_slice(), &[0xff]] {
        fs::write(nested.join("pubspec.yaml"), manifest).unwrap();
        let packages = falcon_analyze::package_identities([
            outer_file.as_path(),
            nested_file.as_path(),
            nested_file.as_path(),
        ]);
        let outer: Vec<_> = packages
            .iter()
            .filter(|package| package.lib_root == outer_root)
            .collect();
        let inner: Vec<_> = packages
            .iter()
            .filter(|package| package.lib_root == nested_root)
            .collect();
        assert_eq!(outer.len(), 1);
        assert_eq!(outer[0].name, "outer");
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].name, "");
    }
}

#[cfg(unix)]
#[test]
fn package_discovery_deduplicates_real_and_symlinked_package_paths() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("package");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(root.join("pubspec.yaml"), "name: shared\n").unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&root, &link).unwrap();
    let real_file = lib.join("main.dart");
    let linked_file = link.join("lib/main.dart");
    let packages = falcon_analyze::package_identities([real_file.as_path(), linked_file.as_path()]);
    let discovered: Vec<_> = packages
        .iter()
        .filter(|package| package.name == "shared")
        .collect();
    assert_eq!(discovered.len(), 1);
    assert_eq!(
        discovered[0].lib_root,
        falcon_analyze::canonical_or_lexical(&lib)
    );
}

#[test]
fn workspace_reports_include_member_configs_clean_counts_and_lsp_parity() {
    for workspace in ["pub", "melos", "melos7"] {
        let temp = tempdir().unwrap();
        let root = temp.path();
        if workspace == "melos" {
            fs::write(root.join("melos.yaml"), "packages:\n  - packages/*\n").unwrap();
        } else {
            fs::write(
                root.join("pubspec.yaml"),
                if workspace == "melos7" {
                    "workspace:\n  - packages/flagged\n  - packages/clean\nmelos:\n  useRootAsPackage: false\n"
                } else {
                    "workspace:\n  - packages/flagged\n  - packages/clean\n"
                },
            )
            .unwrap();
        }
        let config_path = root.join("falcon.json");
        fs::write(
            &config_path,
            r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false}}}"#,
        )
        .unwrap();
        let source = "dynamic value = 1;\n";
        for (name, level) in [("flagged", "error"), ("clean", "off")] {
            let package = root.join("packages").join(name);
            fs::create_dir_all(package.join("lib")).unwrap();
            fs::write(
                package.join("pubspec.yaml"),
                format!("name: {name}\nresolution: workspace\n"),
            )
            .unwrap();
            fs::write(package.join("lib/main.dart"), source).unwrap();
            fs::write(
                package.join("falcon.json"),
                serde_json::json!({
                    "linter": { "rules": {
                        "recommended": false,
                        "suspicious": { "avoid-dynamic": level }
                    }},
                    "cross-file": { "rules": { "recommended": false } }
                })
                .to_string(),
            )
            .unwrap();
        }
        let output = collect_check(&CheckOptions {
            paths: vec![root.to_path_buf()],
            config_path: Some(config_path.clone()),
            max_errors: Some(0),
            quiet: true,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(output.total_files, 2);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].rule, "avoid-dynamic");
        assert_eq!(output.exit_code, 1);
        assert_eq!(output.packages.len(), 2);
        let clean = output.packages.iter().find(|p| p.name == "clean").unwrap();
        let flagged = output
            .packages
            .iter()
            .find(|p| p.name == "flagged")
            .unwrap();
        assert_eq!(
            (clean.total_files, clean.errors, clean.warnings, clean.infos),
            (1, 0, 0, 0)
        );
        assert_eq!(
            (
                flagged.total_files,
                flagged.errors,
                flagged.warnings,
                flagged.infos
            ),
            (1, 1, 0, 0)
        );

        let render = |format, diagnostics: &[falcon_diagnostics::Diagnostic]| {
            falcon_cli::report::render_with_packages(
                format,
                diagnostics,
                &output.root,
                &output.packages,
            )
        };
        let json: Vec<serde_json::Value> =
            serde_json::from_str(&render(OutputFormat::Json, &output.diagnostics)).unwrap();
        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["package"], "flagged");
        assert_eq!(json[0]["rule"], "avoid-dynamic");
        assert_eq!(json[0]["severity"], "error");
        let text = render(OutputFormat::Text, &output.diagnostics);
        assert!(text.contains("Package flagged:"));
        assert!(text.contains("Package clean:"));
        let (groups, summary) = text.split_once("Package summary:\n").unwrap();
        assert!(groups.contains("avoid-dynamic"));
        assert!(!summary.contains("avoid-dynamic"));
        assert!(summary.contains("flagged: 1 files, 1 errors, 0 warnings, 0 infos"));
        assert!(summary.contains("clean: 1 files, 0 errors, 0 warnings, 0 infos"));
        assert_eq!(text.matches("avoid-dynamic").count(), 1);

        // Display limits must not suppress clean members or change full counts.
        let truncated: Vec<serde_json::Value> =
            serde_json::from_str(&render(OutputFormat::Json, &[])).unwrap();
        assert!(truncated.is_empty());
        let truncated_text = render(OutputFormat::Text, &[]);
        assert_eq!(
            truncated_text.split_once("Package summary:\n").unwrap().1,
            summary
        );
        let sarif: serde_json::Value =
            serde_json::from_str(&render(OutputFormat::Sarif, &output.diagnostics)).unwrap();
        assert_eq!(
            sarif["runs"][0]["results"][0]["properties"]["package"],
            "flagged"
        );
        let packages = sarif["runs"][0]["properties"]["packages"]
            .as_array()
            .unwrap();
        assert_eq!(packages.len(), 2);
        for package in packages {
            assert_eq!(package["total_files"], 1);
            assert!(!std::path::Path::new(package["root"].as_str().unwrap()).is_absolute());
        }
        assert_eq!(
            packages.iter().find(|p| p["name"] == "flagged").unwrap()["errors"],
            1
        );
        assert_eq!(
            packages.iter().find(|p| p["name"] == "clean").unwrap()["errors"],
            0
        );

        let mut lsp = LspState::new_with_sdk_locator(Some(config_path), |_| None);
        for (name, expected) in [("flagged", 1), ("clean", 0)] {
            let path = root.join("packages").join(name).join("lib/main.dart");
            let uri = format!("file://{}", path.display());
            let diagnostics = lsp.open(&uri, source.to_string(), Some(1));
            assert_eq!(
                diagnostics
                    .iter()
                    .filter(|d| d.rule == "avoid-dynamic")
                    .count(),
                expected,
                "{name}, workspace={workspace}",
            );
        }
    }
}

fn workspace_report_fixture(root: &std::path::Path) {
    fs::write(
        root.join("pubspec.yaml"),
        "workspace:\n  - packages/flagged\n  - packages/clean\n",
    )
    .unwrap();
    fs::write(
        root.join("falcon.json"),
        r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},"cross-file":{"rules":{"recommended":false}}}"#,
    )
    .unwrap();
    for name in ["flagged", "clean"] {
        let package = root.join("packages").join(name);
        fs::create_dir_all(package.join("lib")).unwrap();
        fs::write(
            package.join("pubspec.yaml"),
            format!("name: {name}\nresolution: workspace\n"),
        )
        .unwrap();
        fs::write(
            package.join("lib/main.dart"),
            if name == "flagged" {
                "dynamic existing = 1;\n"
            } else {
                "int existing = 1;\n"
            },
        )
        .unwrap();
    }
}

#[test]
fn member_only_reports_omit_unselected_siblings_and_nested_nonmembers() {
    let temp = tempdir().unwrap();
    workspace_report_fixture(temp.path());
    let member = temp.path().join("packages/flagged");
    let nested = member.join("example");
    fs::create_dir_all(nested.join("lib")).unwrap();
    fs::write(nested.join("pubspec.yaml"), "name: example\n").unwrap();
    fs::write(nested.join("lib/main.dart"), "dynamic nested = 1;\n").unwrap();
    let output = collect_check(&CheckOptions {
        paths: vec![member.join("lib/../")],
        config_path: Some(temp.path().join("falcon.json")),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(output.packages.len(), 1);
    assert_eq!(output.packages[0].name, "flagged");
    assert_eq!(output.packages[0].total_files, 1);
    assert_eq!(output.packages[0].errors, 1);
    let report: serde_json::Value =
        serde_json::from_str(&falcon_cli::report::render_with_packages(
            OutputFormat::Json,
            &output.diagnostics,
            &output.root,
            &output.packages,
        ))
        .unwrap();
    let nested = report
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| {
            diagnostic["file_path"]
                .as_str()
                .unwrap()
                .contains("example")
        })
        .unwrap();
    assert!(nested["package"].is_null());
    let text = falcon_cli::report::render_with_packages(
        OutputFormat::Text,
        &output.diagnostics,
        &output.root,
        &output.packages,
    );
    assert!(text.contains("Outside workspace packages:"));
    assert!(text.rfind("avoid-dynamic").unwrap() < text.find("Package summary:\n").unwrap());
    assert!(!text.ends_with('\n'));
}

#[cfg(unix)]
#[test]
fn symlinked_workspace_inputs_retain_package_attribution() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    workspace_report_fixture(&workspace);
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&workspace, &link).unwrap();
    let output = collect_check(&CheckOptions {
        paths: vec![link.clone()],
        config_path: Some(link.join("falcon.json")),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        output.packages.iter().map(|p| p.total_files).sum::<usize>(),
        2
    );
    assert_eq!(output.packages.iter().map(|p| p.errors).sum::<usize>(), 1);
}

// Re-executing this test isolates cwd and captures the actual printing path
// without mutating the parent test process's global environment.
#[test]
fn workspace_report_subprocess_entrypoint() {
    let Some(root) = std::env::var_os("FALCON_WORKSPACE_REPORT_TEST") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let format = match std::env::var("FALCON_WORKSPACE_REPORT_FORMAT")
        .unwrap()
        .as_str()
    {
        "json" => OutputFormat::Json,
        "text" => OutputFormat::Text,
        _ => unreachable!(),
    };
    println!("FALCON_REPORT_START");
    let exit_code = run_check(CheckOptions {
        paths: vec![root.clone()],
        format,
        baseline: Some(root.join("baseline.json")),
        max_errors: Some(if format == OutputFormat::Json { 1 } else { 0 }),
        ..Default::default()
    });
    println!("FALCON_REPORT_END:{exit_code}");
}

#[test]
fn explicit_workspace_stdout_counts_after_baseline_before_print_limit() {
    let workspace = tempdir().unwrap();
    let outside = tempdir().unwrap();
    workspace_report_fixture(workspace.path());
    let baseline = workspace.path().join("baseline.json");
    collect_check(&CheckOptions {
        paths: vec![workspace.path().to_path_buf()],
        config_path: Some(workspace.path().join("falcon.json")),
        write_baseline: Some(baseline),
        ..Default::default()
    })
    .unwrap();
    fs::write(
        workspace.path().join("packages/flagged/lib/main.dart"),
        "dynamic existing = 1;\ndynamic introduced = 2;\ndynamic another = 3;\n",
    )
    .unwrap();
    for format in ["json", "text"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "workspace_report_subprocess_entrypoint",
                "--nocapture",
            ])
            .current_dir(outside.path())
            .env("HOME", outside.path())
            .env("FALCON_WORKSPACE_REPORT_TEST", workspace.path())
            .env("FALCON_WORKSPACE_REPORT_FORMAT", format)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let report = stdout.split_once("FALCON_REPORT_START\n").unwrap().1;
        let (report, exit) = report.split_once("FALCON_REPORT_END:").unwrap();
        assert!(exit.starts_with('1'), "{stdout}");
        if format == "json" {
            let report: Vec<serde_json::Value> = serde_json::from_str(report).unwrap();
            assert_eq!(report.len(), 1);
            assert_eq!(report[0]["package"], "flagged");
        } else {
            assert!(report.contains("Package flagged:"));
            let summary = report.split_once("Package summary:\n").unwrap().1;
            assert!(summary.contains("flagged: 1 files, 2 errors, 0 warnings, 0 infos"));
            assert!(summary.contains("clean: 1 files, 0 errors, 0 warnings, 0 infos"));
            assert!(report.contains("Package clean:"));
            assert!(!report.contains("avoid-dynamic"));
        }
    }
}
