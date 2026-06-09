use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "jdlint", about = "A fast Dart linter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
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
