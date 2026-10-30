use clap::Parser;

use crate::analyze_pipeline::run_check;
use crate::args::Cli;

#[must_use]
pub fn run_cli() -> i32 {
    let cli = Cli::parse();
    crate::logging::init(cli.verbose, cli.log_format);

    match cli.command {
        crate::args::Command::Check(options) => run_check(&options),
        crate::args::Command::Migrate {
            input,
            write,
            output,
        } => crate::migrate::run_migrate(input, write, output),
        crate::args::Command::Lsp => match falcon_lsp::run_server() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: LSP server failed: {e}");
                1
            }
        },
        crate::args::Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            0
        }
    }
}
