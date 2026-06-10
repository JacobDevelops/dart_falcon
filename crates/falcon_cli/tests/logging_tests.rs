#[test]
fn test_logging_init_info_does_not_panic() {
    // tracing-subscriber panics if initialized twice; try_init swallows the duplicate
    let _ = tracing_subscriber::fmt::try_init();
}

#[test]
fn test_args_verbose_default_false() {
    use clap::Parser;
    use falcon_cli::args::Cli;
    let args = Cli::try_parse_from(["falcon", "lsp"]).unwrap();
    assert!(!args.verbose);
}

#[test]
fn test_args_verbose_flag() {
    use clap::Parser;
    use falcon_cli::args::Cli;
    let args = Cli::try_parse_from(["falcon", "--verbose", "lsp"]).unwrap();
    assert!(args.verbose);
}

#[test]
fn test_args_check_subcommand() {
    use clap::Parser;
    use falcon_cli::args::{Cli, Command};
    let args = Cli::try_parse_from(["falcon", "check", "."]).unwrap();
    assert!(matches!(args.command, Command::Check { .. }));
}

#[test]
fn test_args_log_format_default_text() {
    use clap::Parser;
    use falcon_cli::args::{Cli, LogFormat};
    let args = Cli::try_parse_from(["falcon", "lsp"]).unwrap();
    assert_eq!(args.log_format, LogFormat::Text);
}

#[test]
fn test_args_log_format_json() {
    use clap::Parser;
    use falcon_cli::args::{Cli, LogFormat};
    let args = Cli::try_parse_from(["falcon", "--log-format", "json", "lsp"]).unwrap();
    assert_eq!(args.log_format, LogFormat::Json);
}

#[test]
fn test_args_log_format_text_explicit() {
    use clap::Parser;
    use falcon_cli::args::{Cli, LogFormat};
    let args = Cli::try_parse_from(["falcon", "--log-format", "text", "lsp"]).unwrap();
    assert_eq!(args.log_format, LogFormat::Text);
}
