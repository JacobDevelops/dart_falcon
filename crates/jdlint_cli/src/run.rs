use clap::Parser;

use crate::analyze_pipeline::{run_check, CheckOptions};
use crate::args::Cli;

pub fn run_cli() -> i32 {
    let cli = Cli::parse();
    crate::logging::init(cli.verbose, cli.log_format);

    match cli.command {
        crate::args::Command::Check {
            paths,
            config,
            format,
            exclude,
            max_errors,
            quiet,
            exit_code,
            parallel,
        } => run_check(CheckOptions {
            paths,
            config_path: config,
            exclude_patterns: exclude,
            max_errors,
            quiet,
            format,
            error_exit_code: exit_code,
            parallel,
        }),
        crate::args::Command::Lsp => {
            tracing::info!("LSP server not yet implemented (M5)");
            0
        }
        crate::args::Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            0
        }
    }
}
