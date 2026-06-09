//! CLI argument parsing and command dispatch.
//!
//! Commands: `jdlint check [paths]`, `jdlint lsp`, `jdlint --version`.

pub mod args;
pub mod logging;
pub mod run;

pub use run::run_cli;
