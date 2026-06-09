use clap::Parser;

use crate::args::Cli;

pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let _cli = Cli::parse();
    todo!("CLI dispatch — implemented in M3")
}
