use clap::{CommandFactory, Parser};
use falcon_cli::args::Cli;
use falcon_cli::{CheckOptions, run_check};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_version_flag_parses() {
    let cmd = Cli::command();
    assert!(cmd.get_version().is_some());
}

#[test]
fn test_check_format_json_parses() {
    let args = vec!["falcon", "check", ".", "--format", "json"];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { format, .. } => {
            assert_eq!(format, falcon_cli::args::OutputFormat::Json);
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_check_exclude_parses() {
    let args = vec![
        "falcon",
        "check",
        ".",
        "--exclude",
        "**/build/**",
        "--exclude",
        "**/.dart_tool/**",
    ];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { exclude, .. } => {
            assert_eq!(exclude.len(), 2);
            assert_eq!(exclude[0], "**/build/**");
            assert_eq!(exclude[1], "**/.dart_tool/**");
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_check_max_errors_parses() {
    let args = vec!["falcon", "check", ".", "--max-errors", "10"];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { max_errors, .. } => {
            assert_eq!(max_errors, Some(10));
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_check_quiet_parses() {
    let args = vec!["falcon", "check", ".", "--quiet"];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { quiet, .. } => {
            assert!(quiet);
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_run_check_integration_clean_file_exit_zero() {
    let dir = tempdir().unwrap();
    // A non-empty body avoids avoid_empty_blocks, uses no magic numbers, and
    // avoids print (now a recommended rule), so a fully clean program exits 0.
    fs::write(
        dir.path().join("test.dart"),
        "void main() {\n  final greeting = 'hello';\n  assert(greeting.isNotEmpty);\n}\n",
    )
    .unwrap();
    let code = run_check(CheckOptions {
        paths: vec![dir.path().to_path_buf()],
        quiet: true,
        ..Default::default()
    });
    assert_eq!(code, 0);
}

#[test]
fn test_check_exit_code_parses() {
    let args = vec!["falcon", "check", ".", "--exit-code", "2"];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { exit_code, .. } => {
            assert_eq!(exit_code, 2);
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_check_exit_code_default_one() {
    let args = vec!["falcon", "check", "."];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { exit_code, .. } => {
            assert_eq!(exit_code, 1);
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_check_parallel_flag_parses() {
    let args = vec!["falcon", "check", ".", "--parallel"];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        falcon_cli::args::Command::Check { parallel, .. } => {
            assert!(parallel);
        }
        _ => panic!("Expected Check command"),
    }
}

#[test]
fn test_version_subcommand_parses() {
    let args = vec!["falcon", "version"];
    let result = Cli::try_parse_from(&args);
    assert!(result.is_ok());
    assert!(matches!(
        result.unwrap().command,
        falcon_cli::args::Command::Version
    ));
}
