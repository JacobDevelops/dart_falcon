use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

pub use crate::analyze_pipeline::OutputFormat;

/// The fully-built clap [`Command`](clap::Command) tree for the `falcon` CLI.
///
/// Exposed so tooling (the docs `cli.json` generator in `xtask`) can walk the
/// real parser definition instead of re-describing the flags by hand.
pub fn command() -> clap::Command {
    Cli::command()
}

#[derive(Parser, Debug)]
#[command(name = "falcon", about = "A fast Dart linter", version)]
pub struct Cli {
    #[arg(long, global = true)]
    pub verbose: bool,
    /// Log output format
    #[arg(long, global = true, default_value = "text", value_name = "FORMAT")]
    pub log_format: LogFormat,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Lint one or more Dart files or directories
    Check(crate::analyze_pipeline::CheckOptions),
    /// Migrate to falcon.json. Converts a dart_code_linter / pyramid_lint
    /// analysis_options.yaml, or upgrades an existing falcon.json by rewriting
    /// legacy rule ids to their canonical form (auto-detected from the input).
    Migrate {
        /// Path to the input: an analysis_options.yaml to convert, or an
        /// existing falcon.json to upgrade (default: ./analysis_options.yaml)
        #[arg(long, value_name = "PATH")]
        input: Option<std::path::PathBuf>,
        /// Write falcon.json instead of printing to stdout
        #[arg(long)]
        write: bool,
        /// Output path when --write is set (default: ./falcon.json)
        #[arg(long, value_name = "PATH")]
        output: Option<std::path::PathBuf>,
    },
    /// Start the LSP server (reads JSON-RPC 2.0 from stdin)
    Lsp,
    /// Print version information and exit
    Version,
}
