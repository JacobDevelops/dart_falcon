//! CLI argument parsing and command dispatch.
//!
//! Commands: `falcon check [paths]`, `falcon lsp`, `falcon version`, `falcon --version`.

pub mod analyze_pipeline;
pub mod args;
pub mod file_walker;
pub mod logging;
pub mod output;
pub mod run;

pub use analyze_pipeline::{run_check, CheckOptions, OutputFormat};
pub use file_walker::walk_files;
pub use output::{format_json, format_text};
pub use run::run_cli;
