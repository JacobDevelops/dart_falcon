use clap::{CommandFactory, Parser};
use falcon_cli::args::{Cli, Command, LogFormat};
use falcon_cli::{CheckOptions, run_check_with_sdk_locator};
use std::fs;
use tempfile::tempdir;

#[test]
fn check_options_parse_with_stable_argument_ids() {
    let command = Cli::command();
    assert!(command.get_version().is_some());
    let check = command.find_subcommand("check").unwrap();
    let ids: Vec<_> = check
        .get_arguments()
        .map(|arg| arg.get_id().as_str())
        .collect();
    for id in [
        "paths",
        "config",
        "format",
        "exclude",
        "max_errors",
        "quiet",
        "exit_code",
        "parallel",
    ] {
        assert!(ids.contains(&id), "missing argument id {id}");
    }
    let config = check
        .get_arguments()
        .find(|arg| arg.get_id() == "config")
        .unwrap();
    assert_eq!(config.get_value_names().unwrap()[0].as_str(), "CONFIG");
    let cli = Cli::try_parse_from([
        "falcon",
        "check",
        ".",
        "--config",
        "falcon.json",
        "--format",
        "json",
        "--exclude",
        "**/build/**",
        "--exclude",
        "**/.dart_tool/**",
        "--max-errors",
        "10",
        "--quiet",
        "--exit-code",
        "2",
        "--parallel",
    ])
    .unwrap();
    let Command::Check(options) = cli.command else {
        panic!("Expected Check command")
    };
    assert_eq!(options.paths, [std::path::PathBuf::from(".")]);
    assert_eq!(options.config_path, Some("falcon.json".into()));
    assert_eq!(options.format, falcon_cli::OutputFormat::Json);
    assert_eq!(
        options.exclude_patterns,
        ["**/build/**", "**/.dart_tool/**"]
    );
    assert_eq!(options.max_errors, Some(10));
    assert!(options.quiet);
    assert_eq!(options.error_exit_code, 2);
    assert!(options.parallel);
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
    let code = run_check_with_sdk_locator(
        CheckOptions {
            paths: vec![dir.path().to_path_buf()],
            quiet: true,
            ..Default::default()
        },
        |_| None,
    );
    assert_eq!(code, 0);
}

#[test]
fn check_and_logging_defaults_and_global_flags() {
    for (flags, verbose, log_format) in [
        (vec![], false, LogFormat::Text),
        (vec!["--verbose"], true, LogFormat::Text),
        (vec!["--log-format", "json"], false, LogFormat::Json),
        (vec!["--log-format", "text"], false, LogFormat::Text),
    ] {
        for subcommand in ["check", "lsp"] {
            let mut argv = vec!["falcon", subcommand];
            argv.extend(flags.iter().copied());
            let cli = Cli::try_parse_from(argv).unwrap();
            assert_eq!(cli.verbose, verbose);
            assert_eq!(cli.log_format, log_format);
            if let Command::Check(options) = cli.command {
                assert_eq!(options.error_exit_code, 1);
                assert_eq!(options.format, falcon_cli::OutputFormat::Text);
                assert!(!options.quiet);
                assert!(!options.parallel);
            } else {
                assert!(matches!(cli.command, Command::Lsp));
            }
        }
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
