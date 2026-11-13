//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, ValueEnum};
use falcon_analyze::{
    AnalysisEnvironment, DartSdk, ExternalIndexParts, ExternalLibraries, ExternalSources,
    IdentityIndex, PackageConfig, ParsedFile, ProjectFile, SdkLocator, SignatureIndex, TypeIndex,
};
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "CheckOptions is a public clap Args type; its boolean fields are part of the public API and CLI flags"
)]
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
    pub partial_fix_write: Option<PartialFixWrite>,
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
    let _analysis = tracing::debug_span!(target: "falcon::stages", "analysis").entered();
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
    let root = paths
        .iter()
        .find_map(|path| {
            let path = cwd.join(falcon_analyze::canonical_or_lexical(path));
            path.ancestors()
                .find(|ancestor| ancestor.join(".git").exists() || ancestor.join(".jj").exists())
                .map(Path::to_path_buf)
        })
        .or_else(|| workspace.as_ref().map(|workspace| workspace.root.clone()))
        .or_else(|| config.root_dir().map(Path::to_path_buf))
        .unwrap_or_else(|| cwd.clone());
    let error_on_warnings = options.error_on_warnings || config.error_on_warnings;
    collect_selected_check(
        options,
        sdk_locator,
        paths,
        workspace.as_ref(),
        &config,
        root,
        error_on_warnings,
    )
}

/// The exact committed and uncommitted paths after a guarded write failure.
#[derive(Debug)]
pub struct PartialFixWrite {
    pub written: Vec<PathBuf>,
    pub not_written: Vec<PathBuf>,
    pub cause: String,
}

impl std::fmt::Display for PartialFixWrite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "could not finish fix writes: {}", self.cause)?;
        writeln!(formatter, "files written:")?;
        for path in &self.written {
            writeln!(formatter, "  {}", path.display())?;
        }
        writeln!(formatter, "files not written:")?;
        for path in &self.not_written {
            writeln!(formatter, "  {}", path.display())?;
        }
        Ok(())
    }
}

fn collect_selected_check(
    options: &CheckOptions,
    sdk_locator: Option<SdkLocator>,
    paths: &[PathBuf],
    workspace: Option<&falcon_project::Workspace>,
    config: &falcon_config::FalconConfig,
    root: PathBuf,
    error_on_warnings: bool,
) -> Result<CheckOutput, String> {
    let cli_filter = check_path_filter(options, config);
    let filter = cli_filter.as_ref().unwrap_or(&config.files.includes);
    let max_errors = options.max_errors.or(config.max_errors);
    let mut analysis = ProjectAnalysis::new(config);
    let walked = tracing::debug_span!(target: "falcon::stages", "discovery")
        .in_scope(|| walk_project(paths, filter, true))?;
    let mut files = walked.files;
    let mut manifests = walked.manifests;
    let mut packages = workspace.as_ref().map_or_else(Vec::new, |workspace| {
        crate::report::summarize_packages(
            &workspace.packages,
            files.iter().map(|(path, _)| path.as_path()),
            &[],
        )
    });
    let mut unmatched = unmatched_diagnostics(
        options,
        paths,
        &files,
        &manifests,
        &walked.missing_paths,
        &walked.filtered_paths,
    );
    let mut environment = capture_environment(sdk_locator, &walked.manifest_paths);
    if files.is_empty() && manifests.is_empty() {
        return unmatched_check_output(options, config, root, unmatched, packages, max_errors);
    }
    // Package configurations give every file its language version; resolving
    // runs also load the SDK and dependency libraries project files import.
    environment.set_project_roots(falcon_project::required_source_roots(
        files.iter().map(|(path, _)| path.as_path()),
    ));
    let mut diff = String::new();
    let mut fix_reports = Vec::new();
    let partial_fix_write = if options.write {
        analysis.set_autofix_snapshots(true);
        let partial = apply_selected_fixes(
            options,
            &analysis,
            config,
            &mut environment,
            (&mut files, &manifests),
            &root,
            (&mut diff, &mut fix_reports),
        )?;
        analysis.set_autofix_snapshots(false);
        partial
    } else {
        None
    };
    let total_files = files.len();
    let FinishedAnalysis {
        mut diagnostics,
        project_files,
        pipeline_failed,
        has_unmatched,
    } = finish_project_analysis(
        &analysis,
        &mut environment,
        config,
        filter,
        &mut files,
        &mut manifests,
        &mut unmatched,
    )?;

    let fix_failed = !fix_reports.is_empty();
    diagnostics.append(&mut fix_reports);
    falcon_project::sort_diagnostics(&mut diagnostics);
    apply_check_baseline(
        options,
        config,
        &root,
        &mut diagnostics,
        &project_files,
        (&files, &manifests),
        analysis.collect_programs(),
    )
    .map_err(|error| baseline_failure_message(error, &diagnostics, partial_fix_write.as_ref()))?;
    crate::report::count_package_diagnostics(&mut packages, &diagnostics);
    let exit_code = check_exit_code(
        options,
        error_on_warnings,
        &diagnostics,
        pipeline_failed || has_unmatched || fix_failed || !diff.is_empty(),
    );
    Ok(CheckOutput {
        diagnostics,
        total_files,
        exit_code,
        max_errors,
        root,
        packages,
        diff,
        partial_fix_write,
    })
}

fn baseline_failure_message(
    mut error: String,
    diagnostics: &[Diagnostic],
    partial: Option<&PartialFixWrite>,
) -> String {
    for diagnostic in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == AUTOFIX_DIAGNOSTIC_ID)
    {
        error.push('\n');
        error.push_str(&diagnostic.file_path);
        error.push_str(": ");
        error.push_str(&diagnostic.message);
    }
    if let Some(partial) = partial {
        error.push('\n');
        error.push_str(&partial.to_string());
    }
    error
}

fn unmatched_check_output(
    options: &CheckOptions,
    config: &falcon_config::FalconConfig,
    root: PathBuf,
    mut diagnostics: Vec<Diagnostic>,
    mut packages: Vec<crate::report::PackageSummary>,
    max_errors: Option<usize>,
) -> Result<CheckOutput, String> {
    crate::adoption::apply_baseline(options, config, &root, &mut diagnostics, &HashMap::new())?;
    crate::report::count_package_diagnostics(&mut packages, &diagnostics);
    Ok(CheckOutput {
        exit_code: i32::from(!diagnostics.is_empty()),
        diagnostics,
        total_files: 0,
        max_errors,
        root,
        packages,
        diff: String::new(),
        partial_fix_write: None,
    })
}

fn capture_environment(
    sdk_locator: Option<SdkLocator>,
    manifest_paths: &[PathBuf],
) -> AnalysisEnvironment {
    let _capture = tracing::debug_span!(target: "falcon::stages", "environment_capture").entered();
    let mut environment = match sdk_locator {
        Some(locator) => AnalysisEnvironment::with_sdk_locator(locator),
        None => AnalysisEnvironment::default(),
    };
    environment.extend(manifest_paths.iter().map(PathBuf::as_path));
    for warning in environment.take_warnings() {
        eprintln!("warning: {warning}");
    }
    environment
}

fn check_path_filter(
    options: &CheckOptions,
    config: &falcon_config::FalconConfig,
) -> Option<falcon_config::PathFilter> {
    if options.exclude_patterns.is_empty() {
        return None;
    }
    let mut filter = config.files.includes.clone();
    filter.extend_excludes(&options.exclude_patterns);
    for warning in &filter.warnings()[config.files.includes.warnings().len()..] {
        eprintln!("warning: {warning}");
    }
    Some(filter)
}

type SourceText = (PathBuf, String);

fn apply_check_baseline(
    options: &CheckOptions,
    config: &falcon_config::FalconConfig,
    root: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    project_files: &[ProjectFile],
    sources: (&[SourceText], &[SourceText]),
    collect_programs: bool,
) -> Result<(), String> {
    if options.baseline.is_some() || config.baseline.is_some() || options.write_baseline.is_some() {
        let (files, manifests) = sources;
        let sources = if collect_programs {
            project_files
                .iter()
                .map(|file| (file.path().to_string_lossy(), file.source()))
                .collect()
        } else {
            files
                .iter()
                .chain(manifests.iter())
                .map(|(path, source)| (path.to_string_lossy(), source.as_str()))
                .collect()
        };
        crate::adoption::apply_baseline(options, config, root, diagnostics, &sources)?;
    }
    Ok(())
}

fn check_exit_code(
    options: &CheckOptions,
    error_on_warnings: bool,
    diagnostics: &[Diagnostic],
    failed: bool,
) -> i32 {
    if failed {
        1
    } else if diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error
            || (error_on_warnings && diagnostic.severity == Severity::Warning)
    }) {
        options.error_exit_code
    } else {
        0
    }
}

fn unmatched_diagnostics(
    options: &CheckOptions,
    paths: &[PathBuf],
    files: &[(PathBuf, String)],
    manifests: &[(PathBuf, String)],
    missing_paths: &[PathBuf],
    filtered_paths: &[PathBuf],
) -> Vec<Diagnostic> {
    let mut unmatched = Vec::new();
    if !options.no_errors_on_unmatched {
        // Ancestor manifests support cross-file analysis but do not select an
        // otherwise empty input directory. Only manifests within inputs count.
        let has_selected_manifest = manifests
            .iter()
            .any(|(manifest, _)| paths.iter().any(|path| manifest.starts_with(path)));
        unmatched.extend(missing_paths.iter().map(|path| {
            Diagnostic::new(
                UNMATCHED_PATH_DIAGNOSTIC_ID,
                Severity::Error,
                "Input path does not exist.",
                path.to_string_lossy(),
                Span { start: 0, end: 0 },
            )
        }));
        unmatched.extend(filtered_paths.iter().map(|path| {
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
    unmatched
}

fn apply_selected_fixes(
    options: &CheckOptions,
    analysis: &ProjectAnalysis,
    config: &falcon_config::FalconConfig,
    environment: &mut AnalysisEnvironment,
    sources: (&mut [SourceText], &[SourceText]),
    root: &Path,
    reports: (&mut String, &mut Vec<Diagnostic>),
) -> Result<Option<PartialFixWrite>, String> {
    let (diff, fix_reports) = reports;
    let (files, manifests) = sources;
    let (initial, mut fix_files, _) =
        analyze_sources(analysis, files, manifests, config, environment)
            .map_err(|error| error.to_string())?;
    let fixable = fixable_paths(initial, config, options.unsafe_fixes);
    if !analysis.resolve() && !fixable.is_empty() {
        load_fix_companions(environment, &mut fix_files).map_err(|error| error.to_string())?;
    }
    let external = (analysis.resolve() && !fixable.is_empty())
        .then(|| FixExternals::load(environment, &mut fix_files))
        .transpose()
        .map_err(|error| error.to_string())?;
    let mut staged = Vec::with_capacity(fixable.len());
    let mut staging_failed = false;
    for (index, (path, source)) in files.iter().enumerate() {
        if !fixable.contains(path.to_string_lossy().as_ref()) {
            continue;
        }
        let snapshot = if options.dry_run {
            None
        } else {
            match WriteSnapshot::capture(path, source) {
                Ok(snapshot) => Some(snapshot),
                Err(error) => {
                    staging_failed |= !matches!(error, CaptureFailure::Ineligible(_));
                    fix_reports.push(fix_report(path, &error.to_string()));
                    continue;
                }
            }
        };
        let outcome = falcon_project::fix_file_with_context(
            analysis,
            config,
            environment,
            path,
            source,
            options.unsafe_fixes,
            falcon_project::FixContext {
                external: external.as_ref().map(FixExternals::indexes),
                external_libraries: external.as_ref().map(|external| &external.libraries),
                project_files: &fix_files,
                cancelled: &|| false,
            },
        );
        if let Some(stop) = outcome.stop {
            staging_failed |= !matches!(stop, falcon_project::FixStop::SyntaxErrors);
            let message = stop.to_string();
            fix_reports.push(fix_report(path, &message));
            continue;
        }
        if outcome.source == *source {
            continue;
        }
        let project_file = fix_files.iter_mut().find(|file| file.path() == path);
        if let Err(message) = WriteSnapshot::apply_with_project_snapshot(
            None,
            path,
            &outcome.source,
            project_file,
            environment,
        ) {
            staging_failed = true;
            fix_reports.push(fix_report(path, &message));
            continue;
        }
        staged.push(StagedFix {
            index,
            source: outcome.source,
            snapshot,
        });
    }
    if staging_failed {
        return Ok(None);
    }
    if options.dry_run {
        for fix in staged {
            let (path, source) = &files[fix.index];
            let relative = crate::baseline::relative_path(&path.to_string_lossy(), root);
            append_full_file_diff(diff, &relative, source, &fix.source);
        }
        return Ok(None);
    }
    let partial = commit_staged_fixes(files, staged).err();
    if let Some(partial) = &partial {
        let message = partial_fix_message(partial);
        for path in &partial.not_written {
            fix_reports.push(fix_report(path, &message));
        }
    }
    Ok(partial)
}

fn partial_fix_message(partial: &PartialFixWrite) -> String {
    format!(
        "write aborted: {}; this file was not written",
        partial.cause
    )
}

fn fixable_paths(
    mut diagnostics: Vec<Diagnostic>,
    config: &falcon_config::FalconConfig,
    unsafe_fixes: bool,
) -> HashSet<String> {
    falcon_rules::apply_severities(&mut diagnostics, config);
    falcon_project::apply_fix_policies(config, &mut diagnostics);
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            diagnostic
                .fix
                .as_ref()
                .is_some_and(|fix| fix.kind == FixKind::Safe || unsafe_fixes)
        })
        .map(|diagnostic| diagnostic.file_path)
        .collect()
}

struct StagedFix {
    index: usize,
    source: String,
    snapshot: Option<WriteSnapshot>,
}

fn commit_staged_fixes(
    files: &mut [SourceText],
    staged: Vec<StagedFix>,
) -> Result<(), PartialFixWrite> {
    let mut written = Vec::with_capacity(staged.len());
    let mut remaining = staged.into_iter();
    while let Some(fix) = remaining.next() {
        let (path, source) = &mut files[fix.index];
        if let Some(snapshot) = fix.snapshot
            && let Err(cause) = snapshot.write(path, &fix.source)
        {
            let mut not_written = Vec::with_capacity(remaining.len() + 1);
            not_written.push(path.clone());
            not_written.extend(remaining.map(|fix| files[fix.index].0.clone()));
            return Err(PartialFixWrite {
                written,
                not_written,
                cause,
            });
        }
        written.push(path.clone());
        *source = fix.source;
    }
    Ok(())
}

struct FinishedAnalysis {
    diagnostics: Vec<Diagnostic>,
    project_files: Vec<ProjectFile>,
    pipeline_failed: bool,
    has_unmatched: bool,
}

fn finish_project_analysis(
    analysis: &ProjectAnalysis,
    environment: &mut AnalysisEnvironment,
    config: &falcon_config::FalconConfig,
    filter: &falcon_config::PathFilter,
    files: &mut Vec<(PathBuf, String)>,
    manifests: &mut Vec<(PathBuf, String)>,
    unmatched: &mut Vec<Diagnostic>,
) -> Result<FinishedAnalysis, String> {
    let collect_programs = analysis.collect_programs();
    let (mut diagnostics, mut project_files, indexes) =
        analyze_sources(analysis, files, manifests, config, environment)
            .map_err(|error| error.to_string())?;
    if collect_programs {
        project_files.extend(
            manifests
                .drain(..)
                .map(|(path, source)| manifest_file(path, source)),
        );
        files.clear();
    }

    let pipeline_failed = complete_analysis(
        environment,
        analysis.cross_file_registry(),
        &project_files,
        config,
        indexes.as_ref(),
        &mut diagnostics,
    );
    let has_unmatched = !unmatched.is_empty();
    diagnostics.append(unmatched);

    for warning in environment.take_warnings() {
        eprintln!("warning: {warning}");
    }

    if collect_programs {
        finalize_diagnostics(
            &mut diagnostics,
            filter,
            project_files
                .iter()
                .map(|file| (file.path(), file.source())),
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
    Ok(FinishedAnalysis {
        diagnostics,
        project_files,
        pipeline_failed,
        has_unmatched,
    })
}

fn fix_report(path: &Path, message: &str) -> Diagnostic {
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

struct FixExternals {
    libraries: ExternalLibraries,
    identities: Arc<IdentityIndex>,
    types: Arc<TypeIndex>,
    signatures: Arc<SignatureIndex>,
}

#[cfg(test)]
thread_local! {
    static FIX_EXTERNAL_LOADS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn load_fix_companions(
    environment: &AnalysisEnvironment,
    files: &mut Vec<ProjectFile>,
) -> Result<ExternalLibraries, falcon_analyze::SourceFailure> {
    let paths: HashSet<_> = files
        .iter()
        .map(|file| falcon_analyze::canonical_or_lexical(file.path()))
        .collect();
    let roots: Vec<_> = files
        .iter()
        .map(|file| (file.path(), file.program(), file.source()))
        .collect();
    let libraries = ExternalLibraries::load(environment, &roots, |path| paths.contains(path));
    // A selected part can promote its unopened library companions out of
    // the external graph. Keep those in the shared project snapshot.
    let roots = falcon_project::required_source_roots(paths.iter().map(PathBuf::as_path));
    let required = |path: &Path| environment.project_source_required(path, &roots);
    if let Some(error) = libraries.worker_failure() {
        return Err(error);
    }
    if let Some(error) = libraries.source_failure(required) {
        return Err(error);
    }
    for path in libraries
        .project_companions()
        .filter(|path| !paths.contains(*path))
    {
        let source = match falcon_analyze::acquire_source(path).companion(path) {
            Ok(Some(source)) => source,
            Ok(None) => continue,
            Err(error) if required(path) => return Err(error),
            Err(error) => {
                tracing::warn!("{error}");
                continue;
            }
        };
        let (parsed, _) =
            ParsedFile::try_parse_for_package(source, environment.package_language_version(path))
                .map_err(|(_, error)| falcon_analyze::SourceFailure {
                path: path.to_path_buf(),
                source: Arc::new(std::io::Error::other(format!(
                    "parser worker failed: {error}"
                ))),
            })?;
        files.push(
            ProjectFile::new(path.to_path_buf(), Arc::new(parsed))
                .expect("a successful parse produces a project snapshot"),
        );
    }
    Ok(libraries)
}

impl FixExternals {
    fn load(
        environment: &AnalysisEnvironment,
        files: &mut Vec<ProjectFile>,
    ) -> Result<Self, falcon_analyze::SourceFailure> {
        #[cfg(test)]
        FIX_EXTERNAL_LOADS.with(|loads| loads.set(loads.get() + 1));
        let libraries = load_fix_companions(environment, files)?;
        let external = ExternalSources {
            libraries: &libraries,
            environment,
        };
        let identities = Arc::new(IdentityIndex::build_external(external));
        let mut types = TypeIndex::from_resolved_files(std::iter::empty(), &identities);
        types.add_external(&libraries, &identities);
        let types = Arc::new(types);
        let signatures = Arc::new(SignatureIndex::build(
            &[],
            Some(&libraries),
            &identities,
            &types,
        ));
        Ok(Self {
            libraries,
            identities,
            types,
            signatures,
        })
    }

    fn indexes(&self) -> ExternalIndexParts<'_> {
        ExternalIndexParts {
            identities: &self.identities,
            types: &self.types,
            signatures: &self.signatures,
        }
    }
}

#[derive(Debug)]
enum CaptureFailure {
    Ineligible(&'static str),
    Conflict(&'static str),
    Io(std::io::Error),
}

impl std::fmt::Display for CaptureFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ineligible(message) | Self::Conflict(message) => formatter.write_str(message),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CaptureFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Ineligible(_) | Self::Conflict(_) => None,
        }
    }
}

struct WriteSnapshot {
    len: u64,
    modified: std::time::SystemTime,
    permissions: std::fs::Permissions,
}

impl WriteSnapshot {
    fn capture(path: &Path, source: &str) -> Result<Self, CaptureFailure> {
        let metadata = Self::metadata(path)?;
        let snapshot = Self {
            len: metadata.len(),
            modified: metadata.modified().map_err(CaptureFailure::Io)?,
            permissions: metadata.permissions(),
        };
        // The walker read source before this snapshot. Detect intervening saves.
        let current = std::fs::read_to_string(path).map_err(CaptureFailure::Io)?;
        if current != source {
            return Err(CaptureFailure::Conflict("file changed since it was read"));
        }
        drop(current);
        snapshot.check(path).map_err(|error| match error {
            CaptureFailure::Ineligible(_) => CaptureFailure::Conflict(
                "file changed since it was read (file type or permissions differ)",
            ),
            other => other,
        })?;
        Ok(snapshot)
    }

    fn metadata(path: &Path) -> Result<std::fs::Metadata, CaptureFailure> {
        let metadata = std::fs::symlink_metadata(path).map_err(CaptureFailure::Io)?;
        if metadata.file_type().is_symlink() {
            return Err(CaptureFailure::Ineligible("refusing to rewrite a symlink"));
        }
        if !metadata.is_file() {
            return Err(CaptureFailure::Conflict("path is no longer a regular file"));
        }
        if metadata.permissions().readonly() {
            return Err(CaptureFailure::Ineligible(
                "refusing to rewrite a read-only file",
            ));
        }
        Ok(metadata)
    }

    fn check(&self, path: &Path) -> Result<(), CaptureFailure> {
        let metadata = Self::metadata(path)?;
        let modified = metadata.modified().map_err(CaptureFailure::Io)?;
        if metadata.len() != self.len || modified != self.modified {
            return Err(CaptureFailure::Conflict(
                "file changed since it was read (mtime or length differs)",
            ));
        }
        Ok(())
    }

    fn apply_with_project_snapshot(
        snapshot: Option<Self>,
        path: &Path,
        source: &str,
        project_file: Option<&mut ProjectFile>,
        environment: &AnalysisEnvironment,
    ) -> Result<(), String> {
        // Prepare the snapshot before committing. Preview advances the same
        // in-memory project state without changing disk or report sources.
        let updated = if project_file.is_some() {
            let (parsed, _) = ParsedFile::try_parse_for_package(
                source,
                environment.package_language_version(path),
            )
            .map_err(|(_, error)| format!("could not refresh fixed project snapshot: {error}"))?;
            Some(Arc::new(parsed))
        } else {
            None
        };
        if let Some(snapshot) = snapshot {
            snapshot.write(path, source)?;
        }
        if let (Some(file), Some(parsed)) = (project_file, updated) {
            file.replace_parsed(parsed)
                .expect("a successful parse produces a project snapshot");
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
        self.check(path).map_err(|error| error.to_string())?;
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
#[must_use]
pub fn run_check(options: &CheckOptions) -> i32 {
    run_check_impl(options, None)
}

/// Run the check pipeline with an explicit SDK discovery policy.
pub fn run_check_with_sdk_locator(
    options: &CheckOptions,
    sdk_locator: fn(Option<&PackageConfig>) -> Option<DartSdk>,
) -> i32 {
    run_check_impl(options, Some(sdk_locator))
}

fn run_check_impl(options: &CheckOptions, sdk_locator: Option<SdkLocator>) -> i32 {
    let result = match collect_check_impl(options, sdk_locator) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    if let Some(partial) = &result.partial_fix_write {
        eprintln!("error: {partial}");
    }

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
