//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

use clap::{Args, ValueEnum};
use falcon_analyze::environment::{DartSdk, PackageConfig};
use falcon_analyze::{
    AnalysisEnvironment, ParsedFile, ProjectFile, Workspace, analyze_parallel_collecting_resolving,
};
use falcon_dart_parser::lex_tree;
use falcon_diagnostics::{Diagnostic, LineIndex, Severity, Span};
use falcon_rules::{
    apply_severities, build_cross_file_registry, build_registry, cross_file_requires_resolution,
    load_config, load_or_default, package_identities, requires_resolution, run_cross_file,
};

use crate::file_walker::walk_project;
use crate::output;

/// Output format for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
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
    /// Effective print limit; `diagnostics` always contains every finding.
    pub max_errors: Option<usize>,
}

/// Run analysis and collect results without printing diagnostics.
///
/// # Errors
///
/// Returns an error message if configuration, directory traversal, or source
/// reading fails (including files exceeding the compact source-size limit).
pub fn collect_check(options: &CheckOptions) -> Result<CheckOutput, String> {
    collect_check_with_sdk_locator(options, DartSdk::locate)
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
    let loaded = match &options.config_path {
        Some(path) => load_config(path).map_err(|e| e.to_string())?,
        None => {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("failed to get current directory: {}", e))?;
            load_or_default(&cwd).map_err(|e| e.to_string())?
        }
    };
    for warning in loaded.warnings {
        eprintln!("warning: {warning}");
    }
    let config = loaded.config;
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
    let registry = build_registry(&config);
    let cross_file_registry = build_cross_file_registry(&config);
    let collect_programs = !cross_file_registry.is_empty();
    let resolve = requires_resolution(&registry) || cross_file_requires_resolution(&config);
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
                "unmatched-path",
                Severity::Error,
                "Input path does not exist.",
                path.to_string_lossy(),
                Span { start: 0, end: 0 },
            )
        }));
        unmatched.extend(walked.filtered_paths.iter().map(|path| {
            Diagnostic::new(
                "unmatched-path",
                Severity::Error,
                "Input path is excluded by files.includes or command-line excludes.",
                path.to_string_lossy(),
                Span { start: 0, end: 0 },
            )
        }));
        if files.is_empty() && !has_selected_manifest && unmatched.is_empty() {
            unmatched.push(Diagnostic::new(
                "unmatched-path",
                Severity::Error,
                "No files were processed.",
                paths[0].to_string_lossy(),
                Span { start: 0, end: 0 },
            ));
        }
    }
    let packages = if resolve {
        package_identities(
            files
                .iter()
                .chain(manifests.iter())
                .map(|(path, _)| path.as_path()),
        )
    } else {
        Vec::new()
    };
    if files.is_empty() && manifests.is_empty() {
        return Ok(CheckOutput {
            exit_code: i32::from(!unmatched.is_empty()),
            diagnostics: unmatched,
            total_files: 0,
            max_errors,
        });
    }
    info!(
        file_count = files.len(),
        rule_count = registry.rules().len(),
        cross_file_rule_count = cross_file_registry.rules().len(),
        resolve,
        "starting check"
    );
    // Package configurations give every file its language version; resolving
    // runs also load the SDK and dependency libraries project files import.
    let environment = AnalysisEnvironment::discover_with_sdk_locator(
        files.iter().map(|(path, _)| path.as_path()),
        sdk_locator,
    );
    let workspace = Workspace {
        packages: &packages,
        environment: Some(&environment),
    };
    let (mut diagnostics, mut project_files, indexes) = analyze_parallel_collecting_resolving(
        &registry,
        &files,
        &config,
        collect_programs,
        resolve,
        workspace,
    );
    let total_files = files.len();
    if collect_programs {
        project_files.extend(manifests.drain(..).map(|(path, source)| {
            let tree = lex_tree(&source);
            ProjectFile {
                path,
                parsed: std::sync::Arc::new(ParsedFile {
                    source: source.into_boxed_str(),
                    tree,
                }),
                has_parse_errors: false,
            }
        }));
        files.clear();
    }

    let mut pipeline_failed = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == falcon_analyze::RULES_THREAD_ERROR);
    apply_severities(&mut diagnostics, &config);
    let has_unmatched = !unmatched.is_empty();
    diagnostics.append(&mut unmatched);

    if collect_programs {
        match run_cross_file(
            &cross_file_registry,
            &project_files,
            &config,
            indexes.as_ref(),
        ) {
            Ok(cross_file) => diagnostics.extend(cross_file),
            Err(error) => {
                pipeline_failed = true;
                diagnostics.extend(
                    project_files
                        .first()
                        .map(|file| falcon_analyze::rules_thread_diagnostic(&file.path, &error)),
                );
            }
        }
    }

    diagnostics.retain(|diagnostic| {
        diagnostic.rule == "unmatched-path"
            || diagnostic.rule == falcon_analyze::RULES_THREAD_ERROR
            || if Path::new(&diagnostic.file_path)
                .extension()
                .is_some_and(|ext| ext == "dart")
            {
                filter.admits(&diagnostic.file_path)
            } else {
                !filter.excludes(&diagnostic.file_path)
            }
    });

    // Resolve 1-based line/col for every diagnostic from its file's source, so
    // text and JSON output carry navigable positions rather than byte offsets.
    let sources: HashMap<Cow<'_, str>, &str> = if collect_programs {
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
    let mut lines: HashMap<&str, LineIndex<'_>> = HashMap::new();
    for d in &mut diagnostics {
        if let Some((path, src)) = sources.get_key_value(d.file_path.as_str()) {
            d.resolve_position_in(lines.entry(path).or_insert_with(|| LineIndex::new(src)));
        }
    }

    // File walking order can vary; sort diagnostics so output and max_errors
    // truncation remain stable across runs.
    // Syntax errors sort ahead of lints within a file (`false` < `true`).
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then((a.rule != "syntax-error").cmp(&(b.rule != "syntax-error")))
            .then(a.span.start.cmp(&b.span.start))
            .then(a.rule.cmp(b.rule))
            .then(a.span.end.cmp(&b.span.end))
            .then(a.message.cmp(&b.message))
    });

    let exit_code = if pipeline_failed || has_unmatched {
        1
    } else if diagnostics.is_empty() {
        0
    } else {
        options.error_exit_code
    };
    Ok(CheckOutput {
        diagnostics,
        total_files,
        exit_code,
        max_errors,
    })
}

/// Run the check pipeline and print diagnostics. Returns 0 if no diagnostics,
/// `error_exit_code` if any found, 1 on pipeline errors.
pub fn run_check(options: CheckOptions) -> i32 {
    run_check_with_sdk_locator(options, DartSdk::locate)
}

/// Run the check pipeline with an explicit SDK discovery policy.
pub fn run_check_with_sdk_locator(
    options: CheckOptions,
    sdk_locator: fn(Option<&PackageConfig>) -> Option<DartSdk>,
) -> i32 {
    let result = match collect_check_with_sdk_locator(&options, sdk_locator) {
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
        }
    }

    result.exit_code
}
