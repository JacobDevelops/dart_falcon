//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::path::PathBuf;
use tracing::{info, warn};

use clap::ValueEnum;
use falcon_analyze::{analyze_parallel, analyze_sequential, RuleRegistry};
use falcon_config::{load_config, load_or_default, FalconConfig};
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_rules::all_rules;

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

/// Result of a check run (plan M3.3.1).
#[derive(Debug)]
pub struct CheckOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub total_files: usize,
    pub exit_code: i32,
}

/// Register every rule that the config does not explicitly disable.
/// Rules absent from `config.rules` default to enabled (Config-as-Contract:
/// the shipped falcon.json lists every rule explicitly).
fn build_registry(config: &FalconConfig) -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    for rule in all_rules() {
        let enabled = config
            .rules
            .get(rule.name())
            .is_none_or(|rc| rc.enabled);
        if enabled {
            registry.register(rule);
        }
    }
    registry
}

/// Apply `config.severity_override` entries to collected diagnostics.
/// Unknown severity names are warned about and skipped.
fn apply_severity_overrides(diagnostics: &mut [Diagnostic], config: &FalconConfig) {
    if config.severity_override.is_empty() {
        return;
    }
    for diag in diagnostics.iter_mut() {
        if let Some(name) = config.severity_override.get(diag.rule) {
            match name.parse::<Severity>() {
                Ok(severity) => diag.severity = severity,
                Err(e) => warn!(rule = diag.rule, "invalid severity override: {}", e),
            }
        }
    }
}

/// Run analysis and collect results without printing diagnostics.
///
/// # Errors
///
/// Returns an error message if the explicit `--config` file cannot be loaded
/// or the current directory is inaccessible.
pub fn collect_check(options: &CheckOptions) -> Result<CheckOutput, String> {
    let config = match &options.config_path {
        Some(path) => load_config(path).map_err(|e| e.to_string())?,
        None => {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("failed to get current directory: {}", e))?;
            load_or_default(&cwd)
        }
    };

    // Config exclude patterns and CLI --exclude patterns are unioned.
    let mut exclude_patterns = config.exclude_patterns.clone();
    exclude_patterns.extend(options.exclude_patterns.iter().cloned());

    let files = walk_files(&options.paths, &exclude_patterns);
    if files.is_empty() {
        return Ok(CheckOutput {
            diagnostics: vec![],
            total_files: 0,
            exit_code: 0,
        });
    }

    let registry = build_registry(&config);
    info!(
        file_count = files.len(),
        rule_count = registry.rules().len(),
        "starting check"
    );
    let mut diagnostics = if options.parallel {
        analyze_parallel(&registry, &files, &config)
    } else {
        analyze_sequential(&registry, &files, &config)
    };

    apply_severity_overrides(&mut diagnostics, &config);

    // Parallel analysis collects in nondeterministic file order; sort so
    // output (and max_errors truncation) is stable across runs and modes.
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.span.start.cmp(&b.span.start))
            .then(a.rule.cmp(b.rule))
    });

    // CLI flag takes precedence over the config value.
    if let Some(max) = options.max_errors.or(config.max_errors) {
        diagnostics.truncate(max);
    }

    let exit_code = if diagnostics.is_empty() {
        0
    } else {
        options.error_exit_code
    };
    Ok(CheckOutput {
        diagnostics,
        total_files: files.len(),
        exit_code,
    })
}

/// Run the check pipeline and print diagnostics. Returns 0 if no diagnostics,
/// `error_exit_code` if any found, 1 on pipeline errors.
pub fn run_check(options: CheckOptions) -> i32 {
    let result = match collect_check(&options) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    if result.total_files == 0 {
        eprintln!("No .dart files found");
        return result.exit_code;
    }

    if !options.quiet {
        match options.format {
            OutputFormat::Text => {
                let text = output::format_text(&result.diagnostics);
                if !text.is_empty() {
                    println!("{}", text);
                }
            }
            OutputFormat::Json => println!("{}", output::format_json(&result.diagnostics)),
        }
    }

    result.exit_code
}
