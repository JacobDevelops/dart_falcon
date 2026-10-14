//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::path::PathBuf;
use tracing::info;

use std::collections::HashMap;

use clap::{Args, ValueEnum};
use falcon_analyze::{
    ProjectFile, analyze_parallel_collecting_resolving, analyze_sequential_collecting_resolving,
};
use falcon_config::{load_config, load_or_default};
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    apply_severities, build_cross_file_registry, build_registry, package_identities,
    requires_resolution, run_cross_file,
};

use crate::file_walker::{walk_files, walk_pubspecs};
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
    /// Paths to lint
    pub paths: Vec<PathBuf>,
    /// Path to falcon.json config
    #[arg(id = "config", long = "config", value_name = "CONFIG")]
    pub config_path: Option<PathBuf>,
    /// Output format for diagnostics
    #[arg(long, default_value = "text", value_name = "FORMAT")]
    pub format: OutputFormat,
    /// Glob patterns to exclude (repeatable)
    #[arg(id = "exclude", long = "exclude", value_name = "GLOB")]
    pub exclude_patterns: Vec<String>,
    /// Stop after this many diagnostics
    #[arg(long, value_name = "N")]
    pub max_errors: Option<usize>,
    /// Suppress all diagnostic output
    #[arg(long)]
    pub quiet: bool,
    /// Exit code to use when violations are found (default: 1)
    #[arg(
        id = "exit_code",
        long = "exit-code",
        value_name = "N",
        default_value_t = 1
    )]
    pub error_exit_code: i32,
    /// Use Rayon parallel file analysis instead of sequential
    #[arg(long)]
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

/// Result of a check run.
#[derive(Debug)]
pub struct CheckOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub total_files: usize,
    pub exit_code: i32,
}

/// Run analysis and collect results without printing diagnostics.
///
/// # Errors
///
/// Returns an error message if the explicit `--config` file cannot be loaded
/// or the current directory is inaccessible.
pub fn collect_check(options: &CheckOptions) -> Result<CheckOutput, String> {
    let mut config = match &options.config_path {
        Some(path) => load_config(path).map_err(|e| e.to_string())?,
        None => {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("failed to get current directory: {}", e))?;
            load_or_default(&cwd).map_err(|e| e.to_string())?
        }
    };
    // Rewrite any legacy rule ids in the config to their canonical ids so old
    // falcon.json files keep resolving.
    falcon_rules::meta::canonicalize_config(&mut config);

    let mut cli_filter;
    let filter = if options.exclude_patterns.is_empty() {
        &config.files.includes
    } else {
        cli_filter = config.files.includes.clone();
        cli_filter.extend_excludes(&options.exclude_patterns);
        &cli_filter
    };
    let files = walk_files(&options.paths, filter);
    let registry = build_registry(&config);
    let cross_file_registry = build_cross_file_registry(&config);
    let collect_programs = !cross_file_registry.is_empty();
    let resolve = requires_resolution(&registry);
    let manifests = if collect_programs || resolve {
        walk_pubspecs(&options.paths, filter)
    } else {
        Vec::new()
    };
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
            diagnostics: vec![],
            total_files: 0,
            exit_code: 0,
        });
    }
    info!(
        file_count = files.len(),
        rule_count = registry.rules().len(),
        cross_file_rule_count = cross_file_registry.rules().len(),
        resolve,
        "starting check"
    );
    let (mut diagnostics, mut project_files, indexes) = if options.parallel {
        analyze_parallel_collecting_resolving(
            &registry,
            &files,
            &config,
            collect_programs,
            resolve,
            &packages,
        )
    } else {
        analyze_sequential_collecting_resolving(
            &registry,
            &files,
            &config,
            collect_programs,
            resolve,
            &packages,
        )
    };
    if collect_programs {
        let (empty_program, _) = falcon_dart_parser::parse("");
        project_files.extend(manifests.iter().map(|(path, source)| ProjectFile {
            path: path.clone(),
            source: source.clone(),
            program: empty_program.clone(),
            has_parse_errors: false,
        }));
    }

    apply_severities(&mut diagnostics, &config);

    if collect_programs {
        diagnostics.extend(run_cross_file(
            &cross_file_registry,
            &project_files,
            &config,
            indexes.as_ref(),
        ));
    }

    // Resolve 1-based line/col for every diagnostic from its file's source, so
    // text and JSON output carry navigable positions rather than byte offsets.
    let sources: HashMap<String, &str> = files
        .iter()
        .chain(manifests.iter())
        .map(|(path, source)| (path.to_string_lossy().into_owned(), source.as_str()))
        .collect();
    for d in &mut diagnostics {
        if let Some(src) = sources.get(&d.file_path) {
            d.resolve_position(src);
        }
    }

    // Parallel analysis collects in nondeterministic file order; sort so
    // output (and max_errors truncation) is stable across runs and modes.
    // Syntax errors sort ahead of lints within a file (`false` < `true`).
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then((a.rule != "syntax-error").cmp(&(b.rule != "syntax-error")))
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
        total_files: files.len() + manifests.len(),
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
