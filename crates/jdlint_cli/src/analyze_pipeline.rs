//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::path::PathBuf;
use tracing::info;

use clap::ValueEnum;
use jdlint_analyze::{analyze_parallel, analyze_sequential, RuleRegistry};
use jdlint_config::{load_config, load_or_default};
use jdlint_rules::all_rules;

use crate::file_walker::walk_files;
use crate::output;

/// Output format for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Options for the check pipeline.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    /// Paths to check (files or directories).
    pub paths: Vec<PathBuf>,
    /// Optional config file path. If None, will search for config.
    pub config_path: Option<PathBuf>,
    /// Glob patterns to exclude from analysis.
    pub exclude_patterns: Vec<String>,
    /// Maximum number of diagnostics to report. None = unlimited.
    pub max_errors: Option<usize>,
    /// If true, suppress all output to stdout.
    pub quiet: bool,
    /// Output format for diagnostics.
    pub format: OutputFormat,
    /// Exit code returned when violations are found. Default: 1.
    pub error_exit_code: i32,
    /// If true, use Rayon parallel analysis; otherwise sequential.
    pub parallel: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            paths: vec![],
            config_path: None,
            exclude_patterns: vec![],
            max_errors: None,
            quiet: false,
            format: OutputFormat::Text,
            error_exit_code: 1,
            parallel: false,
        }
    }
}

/// Run the check pipeline. Returns 0 if no diagnostics, `error_exit_code` if any found.
pub fn run_check(options: CheckOptions) -> i32 {
    let config = match &options.config_path {
        Some(path) => match load_config(path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        },
        None => {
            let cwd = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("error: failed to get current directory: {}", e);
                    return 1;
                }
            };
            load_or_default(&cwd)
        }
    };

    let files = walk_files(&options.paths, &options.exclude_patterns);
    if files.is_empty() {
        eprintln!("No .dart files found");
        return 0;
    }

    let mut registry = RuleRegistry::new();
    for rule in all_rules() {
        registry.register(rule);
    }
    info!(file_count = files.len(), "starting check");
    let mut diagnostics = if options.parallel {
        analyze_parallel(&registry, &files, &config)
    } else {
        analyze_sequential(&registry, &files, &config)
    };

    if let Some(max) = options.max_errors {
        diagnostics.truncate(max);
    }

    if !options.quiet {
        match options.format {
            OutputFormat::Text => {
                let text = output::format_text(&diagnostics);
                if !text.is_empty() {
                    println!("{}", text);
                }
            }
            OutputFormat::Json => println!("{}", output::format_json(&diagnostics)),
        }
    }

    if diagnostics.is_empty() { 0 } else { options.error_exit_code }
}
