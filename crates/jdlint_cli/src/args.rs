use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "jdlint", about = "A fast Dart linter")]
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
        /// Path to jdlint.json config
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
    /// Start the LSP server (reads JSON-RPC 2.0 from stdin)
    Lsp,
}
