//! Integration tests for the analyze pipeline.

use jdlint_cli::{run_check, CheckOptions, OutputFormat};
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

/// Test 2: Dart file found but no rules registered returns zero
#[test]
fn test_run_check_with_dart_file_no_rules_returns_zero() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.dart"), "void main() {}").unwrap();
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
    fs::write(temp.path().join("test.dart"), "void main() {}").unwrap();
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
    fs::write(temp.path().join("test.dart"), "void main() {}").unwrap();
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
        config_path: Some(PathBuf::from("/nonexistent/jdlint.json")),
        quiet: true,
        ..Default::default()
    });
    assert_eq!(exit_code, 1);
}

/// Test 7: --parallel flag runs analysis and still returns zero (no rules)
#[test]
fn test_run_check_parallel_flag_no_rules_zero() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.dart"), "void main() {}").unwrap();
    fs::write(temp.path().join("b.dart"), "class Foo {}").unwrap();
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
    fs::write(temp.path().join("test.dart"), "void main() {}").unwrap();
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
    let result = jdlint_cli::format_json(&[]);
    insta::assert_snapshot!(result);
}

/// Test 10: Text output snapshot for empty diagnostics
#[test]
fn test_output_text_empty_snapshot() {
    let result = jdlint_cli::format_text(&[]);
    insta::assert_snapshot!(result);
}
