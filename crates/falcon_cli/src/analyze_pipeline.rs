//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use falcon_analyze::{AnalysisEnvironment, DartSdk, PackageConfig, SdkLocator};
use falcon_diagnostics::{
    AUTOFIX_DIAGNOSTIC_ID, Diagnostic, FixKind, Severity, Span, UNMATCHED_PATH_DIAGNOSTIC_ID,
};
use falcon_project::{
    ProjectAnalysis, analyze_sources, complete_analysis, finalize_diagnostics,
    load_workspace_config, manifest_file, walk_project,
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
    /// Apply safe fixes to selected Dart files
    #[arg(long)]
    pub write: bool,
    /// Also apply fixes classified as unsafe (requires --write)
    #[arg(long = "unsafe", requires = "write")]
    pub unsafe_fixes: bool,
    /// Preview fixes as a unified diff without modifying files (requires --write)
    #[arg(long, requires = "write", conflicts_with = "write_baseline")]
    pub dry_run: bool,
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
    /// Record current findings (defaults to the configured baseline or project-root falcon-baseline.json)
    #[arg(long, value_name = "PATH", num_args = 0..=1)]
    pub write_baseline: Option<Option<PathBuf>>,
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
            write: false,
            unsafe_fixes: false,
            dry_run: false,
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
    /// Workspace member counts, computed before diagnostic print limits.
    pub packages: Vec<crate::report::PackageSummary>,
    /// Unified diff preview; machine-readable reports send it to stderr.
    pub diff: String,
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
    if (options.unsafe_fixes || options.dry_run) && !options.write {
        return Err("--unsafe and --dry-run require --write".to_owned());
    }
    if options.dry_run && options.write_baseline.is_some() {
        return Err("--dry-run cannot be combined with --write-baseline".to_owned());
    }
    let cwd =
        std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?;
    let input_workspace = options.paths.iter().find_map(|path| {
        let path = falcon_analyze::canonical_or_lexical(path);
        let directory = if path.is_dir() {
            path.as_path()
        } else {
            path.parent()?
        };
        falcon_project::discover_workspace(directory)
    });
    let cwd_workspace = falcon_project::discover_workspace(&cwd);
    let default_paths;
    let paths = if options.paths.is_empty() {
        default_paths = [PathBuf::from(".")];
        &default_paths[..]
    } else {
        &options.paths
    };
    let workspace = input_workspace
        .or_else(|| cwd_workspace.clone())
        .or_else(|| {
            options
                .config_path
                .as_deref()
                .and_then(std::path::Path::parent)
                .and_then(falcon_project::discover_workspace)
        })
        .map(|mut workspace| {
            let inputs: Vec<_> = paths
                .iter()
                .map(|path| falcon_analyze::canonical_or_lexical(path))
                .collect();
            workspace.packages.retain(|package| {
                inputs.iter().any(|input| {
                    if package.root.starts_with(input) {
                        return true;
                    }
                    if !input.starts_with(&package.root) {
                        return false;
                    }
                    let directory = if input.is_dir() {
                        input.as_path()
                    } else {
                        input.parent().unwrap_or(input)
                    };
                    directory
                        .ancestors()
                        .find(|ancestor| ancestor.join("pubspec.yaml").is_file())
                        .is_some_and(|nearest| nearest == package.root)
                })
            });
            workspace
        });
    // Inputs select members, but only --config is an explicit configuration.
    // Discovered cwd configuration takes precedence over the input workspace.
    let loaded = load_workspace_config(
        options.config_path.as_deref(),
        &cwd,
        workspace.as_ref(),
        cwd_workspace.as_ref(),
    )
    .map_err(|e| e.to_string())?;
    for warning in loaded.warnings {
        eprintln!("warning: {warning}");
    }
    let config = loaded.config;
    let root = workspace
        .as_ref()
        .map(|workspace| workspace.root.clone())
        .or_else(|| config.root_dir().map(Path::to_path_buf))
        .or_else(|| {
            paths.iter().find_map(|path| {
                let path = falcon_analyze::canonical_or_lexical(path);
                path.ancestors()
                    .find(|ancestor| ancestor.join(".git").exists())
                    .map(Path::to_path_buf)
            })
        })
        .unwrap_or_else(|| cwd.clone());
    let error_on_warnings = options.error_on_warnings || config.error_on_warnings;

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
    let mut packages = workspace.as_ref().map_or_else(Vec::new, |workspace| {
        crate::report::summarize_packages(
            &workspace.packages,
            files.iter().map(|(path, _)| path.as_path()),
            &[],
        )
    });
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
        crate::report::count_package_diagnostics(&mut packages, &unmatched);
        return Ok(CheckOutput {
            exit_code: i32::from(!unmatched.is_empty()),
            diagnostics: unmatched,
            total_files: 0,
            max_errors,
            root,
            packages,
            diff: String::new(),
        });
    }
    // Package configurations give every file its language version; resolving
    // runs also load the SDK and dependency libraries project files import.
    let mut environment = match sdk_locator {
        Some(locator) => AnalysisEnvironment::with_sdk_locator(locator),
        None => AnalysisEnvironment::default(),
    };
    let mut diff = String::new();
    let mut fix_reports = Vec::new();
    if options.write {
        let (mut initial, _, _) =
            analyze_sources(&analysis, &files, &manifests, &config, &mut environment);
        falcon_rules::apply_severities(&mut initial, &config);
        falcon_project::apply_fix_policies(&config, &mut initial);
        let fixable: HashSet<_> = initial
            .into_iter()
            .filter(|diagnostic| {
                diagnostic
                    .fix
                    .as_ref()
                    .is_some_and(|fix| fix.kind == FixKind::Safe || options.unsafe_fixes)
            })
            .map(|diagnostic| diagnostic.file_path)
            .collect();
        for (path, source) in &mut files {
            if !fixable.contains(path.to_string_lossy().as_ref()) {
                continue;
            }
            let snapshot = if options.dry_run {
                None
            } else {
                match WriteSnapshot::capture(path, source) {
                    Ok(snapshot) => Some(snapshot),
                    Err(message) => {
                        fix_reports.push(fix_report(path, message));
                        continue;
                    }
                }
            };
            let outcome = falcon_project::fix_file(
                &analysis,
                &config,
                &mut environment,
                path,
                source,
                options.unsafe_fixes,
            );
            if let Some(stop) = outcome.stop {
                fix_reports.push(fix_report(path, stop.to_string()));
                continue;
            }
            if outcome.source == *source {
                continue;
            }
            if let Some(snapshot) = snapshot {
                match snapshot.write(path, &outcome.source) {
                    Ok(()) => *source = outcome.source,
                    Err(message) => fix_reports.push(fix_report(path, message)),
                }
            } else {
                let relative = crate::baseline::relative_path(&path.to_string_lossy(), &root);
                append_full_file_diff(&mut diff, &relative, source, &outcome.source);
            }
        }
    }
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

    let fix_failed = !fix_reports.is_empty();
    diagnostics.append(&mut fix_reports);
    falcon_project::sort_diagnostics(&mut diagnostics);
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
    crate::report::count_package_diagnostics(&mut packages, &diagnostics);
    let exit_code = if pipeline_failed || has_unmatched || fix_failed || !diff.is_empty() {
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
        packages,
        diff,
    })
}

fn fix_report(path: &Path, message: String) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        AUTOFIX_DIAGNOSTIC_ID,
        Severity::Error,
        format!("Skipped autofix: {message}."),
        path.to_string_lossy(),
        Span { start: 0, end: 0 },
    );
    diagnostic.resolve_position("");
    diagnostic
}

struct WriteSnapshot {
    len: u64,
    modified: std::time::SystemTime,
    permissions: std::fs::Permissions,
}

impl WriteSnapshot {
    fn capture(path: &Path, source: &str) -> Result<Self, String> {
        let metadata = Self::metadata(path)?;
        let snapshot = Self {
            len: metadata.len(),
            modified: metadata.modified().map_err(|error| error.to_string())?,
            permissions: metadata.permissions(),
        };
        // The walker read source before this snapshot. Detect intervening saves.
        let current = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        if current != source {
            return Err("file changed since it was read".to_owned());
        }
        drop(current);
        snapshot.check(path)?;
        Ok(snapshot)
    }

    fn metadata(path: &Path) -> Result<std::fs::Metadata, String> {
        let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("refusing to rewrite a symlink".to_owned());
        }
        if !metadata.is_file() {
            return Err("path is no longer a regular file".to_owned());
        }
        if metadata.permissions().readonly() {
            return Err("refusing to rewrite a read-only file".to_owned());
        }
        Ok(metadata)
    }

    fn check(&self, path: &Path) -> Result<(), String> {
        let metadata = Self::metadata(path)?;
        let modified = metadata.modified().map_err(|error| error.to_string())?;
        if metadata.len() != self.len || modified != self.modified {
            return Err("file changed since it was read (mtime or length differs)".to_owned());
        }
        Ok(())
    }

    fn write(self, path: &Path, source: &str) -> Result<(), String> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| format!("could not create temporary file: {error}"))?;
        temporary
            .write_all(source.as_bytes())
            .map_err(|error| format!("could not write temporary file: {error}"))?;
        temporary
            .as_file()
            .set_permissions(self.permissions.clone())
            .map_err(|error| format!("could not preserve file permissions: {error}"))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| format!("could not sync temporary file: {error}"))?;
        self.check(path)?;
        temporary
            .persist(path)
            .map_err(|error| format!("could not replace file: {error}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

fn append_full_file_diff(output: &mut String, path: &str, before: &str, after: &str) {
    let old_lines = before.lines().count();
    let new_lines = after.lines().count();
    writeln!(output, "--- a/{path}").unwrap();
    writeln!(output, "+++ b/{path}").unwrap();
    writeln!(
        output,
        "@@ -{},{} +{},{} @@",
        usize::from(old_lines != 0),
        old_lines,
        usize::from(new_lines != 0),
        new_lines
    )
    .unwrap();
    for (prefix, source) in [('-', before), ('+', after)] {
        for line in source.split_inclusive('\n') {
            output.push(prefix);
            output.push_str(line);
            if !line.ends_with('\n') {
                output.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
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

    if !result.diff.is_empty() {
        if options.format == OutputFormat::Text {
            print!("{}", result.diff);
        } else {
            eprint!("{}", result.diff);
        }
    }
    let print_count = result
        .max_errors
        .unwrap_or(usize::MAX)
        .min(result.diagnostics.len());
    let printed = &result.diagnostics[..print_count];
    if !options.quiet {
        let rendered = crate::report::render_with_packages(
            options.format,
            printed,
            &result.root,
            &result.packages,
        );
        if !rendered.is_empty() {
            println!("{rendered}");
        }
    }

    result.exit_code
}
