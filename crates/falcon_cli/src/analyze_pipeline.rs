//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::output;
use clap::{Args, ValueEnum};
use falcon_analyze::{AnalysisEnvironment, DartSdk, PackageConfig, SdkLocator};
use falcon_diagnostics::{Diagnostic, Severity, Span, UNMATCHED_PATH_DIAGNOSTIC_ID};
use falcon_project::{
    ProjectAnalysis, analyze_sources, complete_analysis, finalize_diagnostics, load_project_config,
    manifest_file, walk_project,
};

/// Output format for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
    Gitlab,
    Checkstyle,
    Sonar,
    Html,
}

/// Options for the check pipeline.
#[derive(Debug, Clone, Args)]
pub struct CheckOptions {
    /// Paths to lint (defaults to the current directory)
    pub paths: Vec<PathBuf>,
    /// Path to falcon.json config
    #[arg(id = "config", long = "config", value_name = "CONFIG")]
    pub config_path: Option<PathBuf>,
    /// Output format for diagnostics
    #[arg(long, default_value = "text", value_name = "FORMAT")]
    pub format: OutputFormat,
    /// Config-relative glob patterns to exclude (repeatable)
    #[arg(id = "exclude", long = "exclude", value_name = "GLOB")]
    pub exclude_patterns: Vec<String>,
    /// Limit the number of printed diagnostics without changing the exit status
    #[arg(long, value_name = "N")]
    pub max_errors: Option<usize>,
    /// Fail on warnings as well as errors
    #[arg(long)]
    pub error_on_warnings: bool,
    /// Suppress existing findings recorded in this baseline
    #[arg(long, value_name = "PATH")]
    pub baseline: Option<PathBuf>,
    /// Record current findings (defaults to falcon-baseline.json)
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "falcon-baseline.json")]
    pub write_baseline: Option<PathBuf>,
    /// Suppress all diagnostic output
    #[arg(long)]
    pub quiet: bool,
    /// Do not report missing paths or runs that process no files
    #[arg(long)]
    pub no_errors_on_unmatched: bool,
    /// Exit code to use when violations are found (default: 1)
    #[arg(
        id = "exit_code",
        long = "exit-code",
        alias = "error-exit-code",
        value_name = "N",
        default_value_t = 1
    )]
    pub error_exit_code: i32,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            paths: vec![],
            config_path: None,
            exclude_patterns: vec![],
            max_errors: None,
            error_on_warnings: false,
            baseline: None,
            write_baseline: None,
            quiet: false,
            no_errors_on_unmatched: false,
            format: OutputFormat::Text,
            error_exit_code: 1,
        }
    }
}

/// Result of a check run.
#[derive(Debug)]
pub struct CheckOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub total_files: usize,
    pub exit_code: i32,
    /// Effective print limit; `diagnostics` contains every unbaselined finding.
    pub max_errors: Option<usize>,
    /// Root used for portable report and baseline paths.
    pub root: PathBuf,
}

/// Run analysis and collect results without printing diagnostics.
///
/// # Errors
///
/// Returns an error message if configuration, directory traversal, or source
/// reading fails (including files exceeding the compact source-size limit).
pub fn collect_check(options: &CheckOptions) -> Result<CheckOutput, String> {
    collect_check_impl(options, None)
}

/// Collect analysis with an explicit SDK discovery policy.
///
/// # Errors
///
/// Returns the same configuration and directory errors as [`collect_check`].
pub fn collect_check_with_sdk_locator(
    options: &CheckOptions,
    sdk_locator: fn(Option<&PackageConfig>) -> Option<DartSdk>,
) -> Result<CheckOutput, String> {
    collect_check_impl(options, Some(sdk_locator))
}

fn collect_check_impl(
    options: &CheckOptions,
    sdk_locator: Option<SdkLocator>,
) -> Result<CheckOutput, String> {
    let start_dir = if options.config_path.is_none() {
        std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?
    } else {
        PathBuf::from(".")
    };
    let loaded = load_project_config(options.config_path.as_deref(), &start_dir)
        .map_err(|e| e.to_string())?;
    for warning in loaded.warnings {
        eprintln!("warning: {warning}");
    }
    let config = loaded.config;
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let root = config.root_dir().unwrap_or(&cwd).to_path_buf();
    let error_on_warnings = options.error_on_warnings || config.error_on_warnings;
    let default_paths;
    let paths = if options.paths.is_empty() {
        default_paths = [PathBuf::from(".")];
        &default_paths[..]
    } else {
        &options.paths
    };

    let mut cli_filter;
    let filter = if options.exclude_patterns.is_empty() {
        &config.files.includes
    } else {
        cli_filter = config.files.includes.clone();
        cli_filter.extend_excludes(&options.exclude_patterns);
        for warning in &cli_filter.warnings()[config.files.includes.warnings().len()..] {
            eprintln!("warning: {warning}");
        }
        &cli_filter
    };
    let max_errors = options.max_errors.or(config.max_errors);
    let analysis = ProjectAnalysis::new(&config);
    let collect_programs = analysis.collect_programs;
    let walked = walk_project(paths, filter, true)?;
    let mut files = walked.files;
    let mut manifests = walked.manifests;
    let mut unmatched = Vec::new();
    if !options.no_errors_on_unmatched {
        // Ancestor manifests support cross-file analysis but do not select an
        // otherwise empty input directory. Only manifests within inputs count.
        let has_selected_manifest = manifests
            .iter()
            .any(|(manifest, _)| paths.iter().any(|path| manifest.starts_with(path)));
        unmatched.extend(walked.missing_paths.iter().map(|path| {
            Diagnostic::new(
                UNMATCHED_PATH_DIAGNOSTIC_ID,
                Severity::Error,
                "Input path does not exist.",
                path.to_string_lossy(),
                Span { start: 0, end: 0 },
            )
        }));
        unmatched.extend(walked.filtered_paths.iter().map(|path| {
            Diagnostic::new(
                UNMATCHED_PATH_DIAGNOSTIC_ID,
                Severity::Error,
                "Input path is excluded by files.includes or command-line excludes.",
                path.to_string_lossy(),
                Span { start: 0, end: 0 },
            )
        }));
        if files.is_empty() && !has_selected_manifest && unmatched.is_empty() {
            unmatched.push(Diagnostic::new(
                UNMATCHED_PATH_DIAGNOSTIC_ID,
                Severity::Error,
                "No files were processed.",
                paths[0].to_string_lossy(),
                Span { start: 0, end: 0 },
            ));
        }
    }
    if files.is_empty() && manifests.is_empty() {
        crate::adoption::apply_baseline(options, &config, &root, &mut unmatched, &HashMap::new())?;
        return Ok(CheckOutput {
            exit_code: i32::from(!unmatched.is_empty()),
            diagnostics: unmatched,
            total_files: 0,
            max_errors,
            root,
        });
    }
    // Package configurations give every file its language version; resolving
    // runs also load the SDK and dependency libraries project files import.
    let mut environment = match sdk_locator {
        Some(locator) => AnalysisEnvironment::with_sdk_locator(locator),
        None => AnalysisEnvironment::default(),
    };
    let (mut diagnostics, mut project_files, indexes) =
        analyze_sources(&analysis, &files, &manifests, &config, &mut environment);
    let total_files = files.len();
    if collect_programs {
        project_files.extend(
            manifests
                .drain(..)
                .map(|(path, source)| manifest_file(path, source)),
        );
        files.clear();
    }

    let pipeline_failed = complete_analysis(
        &environment,
        &analysis.cross_file_registry,
        &project_files,
        &config,
        indexes.as_ref(),
        &mut diagnostics,
    );
    let has_unmatched = !unmatched.is_empty();
    diagnostics.append(&mut unmatched);

    for warning in environment.take_warnings() {
        eprintln!("warning: {warning}");
    }

    if collect_programs {
        finalize_diagnostics(
            &mut diagnostics,
            filter,
            project_files
                .iter()
                .map(|file| (file.path.as_path(), file.source())),
        );
    } else {
        finalize_diagnostics(
            &mut diagnostics,
            filter,
            files
                .iter()
                .chain(manifests.iter())
                .map(|(path, source)| (path.as_path(), source.as_str())),
        );
    }

    if options.baseline.is_some() || config.baseline.is_some() || options.write_baseline.is_some() {
        let sources = if collect_programs {
            project_files
                .iter()
                .map(|file| (file.path.to_string_lossy(), file.source()))
                .collect()
        } else {
            files
                .iter()
                .chain(manifests.iter())
                .map(|(path, source)| (path.to_string_lossy(), source.as_str()))
                .collect()
        };
        crate::adoption::apply_baseline(options, &config, &root, &mut diagnostics, &sources)?;
    }
    let exit_code = if pipeline_failed || has_unmatched {
        1
    } else if diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error
            || (error_on_warnings && diagnostic.severity == Severity::Warning)
    }) {
        options.error_exit_code
    } else {
        0
    };
    Ok(CheckOutput {
        diagnostics,
        total_files,
        exit_code,
        max_errors,
        root,
    })
}

/// Run the check pipeline and print diagnostics. Errors fail, warnings fail
/// only when requested, and pipeline errors always return 1.
pub fn run_check(options: CheckOptions) -> i32 {
    run_check_impl(options, None)
}

/// Run the check pipeline with an explicit SDK discovery policy.
pub fn run_check_with_sdk_locator(
    options: CheckOptions,
    sdk_locator: fn(Option<&PackageConfig>) -> Option<DartSdk>,
) -> i32 {
    run_check_impl(options, Some(sdk_locator))
}

fn run_check_impl(options: CheckOptions, sdk_locator: Option<SdkLocator>) -> i32 {
    let result = match collect_check_impl(&options, sdk_locator) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("error: {}", e);
            return 1;
        }
    };

    let print_count = result
        .max_errors
        .unwrap_or(usize::MAX)
        .min(result.diagnostics.len());
    let printed = &result.diagnostics[..print_count];
    if !options.quiet {
        match options.format {
            OutputFormat::Text => {
                let text = output::format_text(printed);
                if !text.is_empty() {
                    println!("{}", text);
                }
            }
            OutputFormat::Json => println!("{}", output::format_json(printed)),
            format => println!("{}", crate::report::render(format, printed, &result.root)),
        }
    }

    result.exit_code
}
