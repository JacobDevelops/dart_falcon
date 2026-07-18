use clap::{Parser, Subcommand, ValueEnum};

pub use crate::analyze_pipeline::OutputFormat;

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
    Check {
        /// Paths to lint
        paths: Vec<std::path::PathBuf>,
        /// Path to falcon.json config
        #[arg(long)]
        config: Option<std::path::PathBuf>,
        /// Output format for diagnostics
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: OutputFormat,
        /// Glob patterns to exclude (repeatable)
        #[arg(long, value_name = "GLOB")]
        exclude: Vec<String>,
        /// Stop after this many diagnostics
        #[arg(long, value_name = "N")]
        max_errors: Option<usize>,
        /// Suppress all diagnostic output
        #[arg(long)]
        quiet: bool,
        /// Exit code to use when violations are found (default: 1)
        #[arg(long, value_name = "N", default_value_t = 1)]
        exit_code: i32,
        /// Use Rayon parallel file analysis instead of sequential
        #[arg(long)]
        parallel: bool,
    },
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
