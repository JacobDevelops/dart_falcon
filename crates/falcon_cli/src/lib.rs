//! CLI argument parsing and command dispatch.
//!
//! Commands: `falcon check [paths]`, `falcon migrate`, `falcon lsp`, `falcon version`, `falcon --version`.

pub mod analyze_pipeline;
pub mod args;
pub mod logging;
pub mod migrate;
pub mod output;
pub mod run;

pub use analyze_pipeline::{
    CheckOptions, CheckOutput, OutputFormat, collect_check, collect_check_with_sdk_locator,
    run_check, run_check_with_sdk_locator,
};
pub use migrate::{MigrationResult, migrate_existing_config, migrate_yaml_to_config, run_migrate};
pub use output::{format_json, format_text};
pub use run::run_cli;
