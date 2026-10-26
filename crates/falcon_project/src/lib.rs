//! Project discovery and reusable analysis stages shared by the CLI and LSP.

pub mod autofix;
mod autofix_analysis;
mod discovery;
mod workspace;

pub use autofix::{
    FixContext, FixOutcome, FixStop, apply_fix_policies, effective_fix_kind, fix_file,
    fix_file_with_context,
};
pub use workspace::{Workspace, WorkspacePackage, discover_workspace};

pub use discovery::{
    DiscoveryWatch, ProjectDiscovery, WalkOutput, discover_project, discover_project_with_watches,
    walk_project,
};
pub use falcon_rules::run_cross_file as cross_file_diagnostics;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, CrossFileRuleRegistry, ParsedFile, ProjectFile,
    RuleRegistry, SemanticIndexes, Workspace as AnalysisWorkspace,
    analyze_parallel_collecting_resolving, package_identities,
};
use falcon_config::{ConfigError, FalconConfig, LoadedConfig, PathFilter};
use falcon_diagnostics::{
    Diagnostic, LineIndex, RULES_THREAD_ERROR_DIAGNOSTIC_ID, SYNTAX_ERROR_DIAGNOSTIC_ID,
    UNMATCHED_PATH_DIAGNOSTIC_ID,
};

/// Load an explicit config, or discover one relative to the caller's start directory.
///
/// # Errors
/// Returns configuration read, parse, and validation errors.
pub fn load_project_config(
    explicit: Option<&Path>,
    start_dir: &Path,
) -> Result<LoadedConfig, ConfigError> {
    let workspace = discover_workspace(start_dir);
    load_project_config_impl(
        explicit,
        start_dir,
        workspace.as_ref(),
        workspace.as_ref(),
        Err,
    )
}

/// Load a project while retaining valid configurations when individual members fail.
///
/// Returned member errors include the configuration file path and retain their
/// structured key path. A failed member inherits the nearest valid configuration.
///
/// # Errors
/// Returns an error if the base configuration cannot be read or validated.
pub fn load_project_config_with_errors(
    explicit: Option<&Path>,
    start_dir: &Path,
) -> Result<(LoadedConfig, Vec<ConfigError>), ConfigError> {
    let workspace = discover_workspace(start_dir);
    let mut errors = Vec::new();
    let loaded = load_project_config_impl(
        explicit,
        start_dir,
        workspace.as_ref(),
        workspace.as_ref(),
        |error| {
            errors.push(error);
            Ok(())
        },
    )?;
    Ok((loaded, errors))
}

/// Load configuration for an already selected subset of workspace members.
///
/// Base discovery follows the caller's start workspace or directory, falling back
/// to the input workspace when no configuration is found there. Home configuration
/// paths bind to the input workspace. Only supplied members load their nearest configuration.
/// Pass the unfiltered start-directory workspace to preserve cwd membership
/// without repeating discovery.
///
/// # Errors
/// Returns configuration read, parse, and validation errors.
pub fn load_workspace_config(
    explicit: Option<&Path>,
    start_dir: &Path,
    workspace: Option<&Workspace>,
    start_workspace: Option<&Workspace>,
) -> Result<LoadedConfig, ConfigError> {
    load_project_config_impl(explicit, start_dir, start_workspace, workspace, Err)
}

fn load_project_config_impl(
    explicit: Option<&Path>,
    start_dir: &Path,
    start_workspace: Option<&Workspace>,
    workspace: Option<&Workspace>,
    mut member_error: impl FnMut(ConfigError) -> Result<(), ConfigError>,
) -> Result<LoadedConfig, ConfigError> {
    let explicit_workspace = if workspace.is_none() {
        explicit.and_then(Path::parent).and_then(discover_workspace)
    } else {
        None
    };
    let workspace = workspace.or(explicit_workspace.as_ref());
    let discovery_base = start_workspace.map_or(start_dir, |workspace| workspace.root.as_path());
    let base = workspace.map_or(discovery_base, |workspace| workspace.root.as_path());
    let discovered = if explicit.is_none() {
        falcon_config::find_config(discovery_base)
    } else {
        None
    };
    let mut loaded = match explicit.or(discovered.as_deref()) {
        Some(path) => {
            let mut loaded = falcon_rules::load_config(path)?;
            if falcon_config::is_home_config(path) {
                loaded.config.resolve_paths(base);
            }
            loaded
        }
        None => falcon_rules::load_or_default(base)?,
    };
    if let Some(workspace) = workspace {
        let mut paths = std::collections::BTreeSet::new();
        if explicit.is_none()
            && let Some(local) = falcon_config::find_config(start_dir)
        {
            let local = std::fs::canonicalize(&local).unwrap_or(local);
            if let Some(directory) = local.parent()
                && directory != workspace.root
                && directory.starts_with(&workspace.root)
            {
                let owner = directory
                    .ancestors()
                    .find(|ancestor| ancestor.join("pubspec.yaml").is_file());
                // Membership must be checked before CLI selection: an unselected
                // member's cwd config is not a nonmember local configuration.
                let members = &start_workspace.unwrap_or(workspace).packages;
                let covered = members.iter().any(|package| {
                    package.root != workspace.root
                        && owner.map_or_else(
                            || directory.starts_with(&package.root),
                            |owner| owner == package.root,
                        )
                });
                if !covered {
                    paths.insert(local);
                }
            }
        }
        for package in &workspace.packages {
            for directory in package.root.ancestors() {
                if directory == workspace.root {
                    break;
                }
                let path = directory.join("falcon.json");
                if path.is_file() {
                    paths.insert(path);
                    break;
                }
            }
        }
        let mut configs = Vec::with_capacity(paths.len());
        let explicit_base = explicit.and_then(|path| std::fs::canonicalize(path).ok());
        for path in paths {
            let is_explicit_base = explicit_base
                .as_ref()
                .is_some_and(|base| std::fs::canonicalize(&path).is_ok_and(|path| path == *base));
            let package = match load_member_config(&path, is_explicit_base) {
                Ok(package) => package,
                Err(error) => {
                    member_error(ConfigError(
                        format!("{}: {}", path.display(), error.0),
                        error.1,
                    ))?;
                    continue;
                }
            };
            loaded.warnings.extend(
                package
                    .warnings
                    .into_iter()
                    .map(|warning| format!("{}: {warning}", path.display())),
            );
            configs.push(package.config);
        }
        loaded.config.set_package_configs(configs);
    }
    Ok(loaded)
}

fn load_member_config(path: &Path, is_explicit_base: bool) -> Result<LoadedConfig, ConfigError> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| ConfigError::new(format!("failed to read config file: {error}")))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| ConfigError::new(format!("failed to parse config JSON: {error}")))?;
    let mut loaded = falcon_config::deserialize_config_value(&value, path)
        .and_then(falcon_rules::canonicalize_loaded_config)?;
    for key in ["max-errors", "max_errors", "error-on-warnings", "baseline"] {
        if !is_explicit_base && value.get(key).is_some() {
            loaded.warnings.push(format!(
                "member configuration ignores run-wide setting `{key}`; \
                 set it in the base configuration or use a CLI override"
            ));
        }
    }
    Ok(loaded)
}

/// Enabled rules and the project facts they require.
pub struct ProjectAnalysis {
    pub registry: RuleRegistry,
    pub cross_file_registry: CrossFileRuleRegistry,
    pub resolve: bool,
    pub collect_programs: bool,
    pub cross_file_resolve: bool,
}

impl ProjectAnalysis {
    pub fn new(config: &FalconConfig) -> Self {
        let registry = falcon_rules::build_registry(config);
        let cross_file_registry = falcon_rules::build_cross_file_registry(config);
        let cross_file_resolve = falcon_rules::cross_file_requires_resolution(config);
        let resolve = falcon_rules::requires_resolution(&registry) || cross_file_resolve;
        let collect_programs = !cross_file_registry.is_empty();
        Self {
            registry,
            cross_file_registry,
            resolve,
            cross_file_resolve,
            collect_programs,
        }
    }
}

/// Analyze source strings, retaining parsed files and semantic indexes when required.
/// Callers own the environment so they can retain it and deliver its warnings.
pub fn analyze_sources(
    analysis: &ProjectAnalysis,
    files: &[(PathBuf, String)],
    manifests: &[(PathBuf, String)],
    config: &FalconConfig,
    environment: &mut AnalysisEnvironment,
) -> (Vec<Diagnostic>, Vec<ProjectFile>, Option<SemanticIndexes>) {
    let packages = if analysis.resolve {
        package_identities(
            files
                .iter()
                .chain(manifests)
                .map(|(path, _)| path.as_path()),
        )
    } else {
        Vec::new()
    };
    environment.extend(files.iter().map(|(path, _)| path.as_path()));
    tracing::info!(
        file_count = files.len(),
        rule_count = analysis.registry.rules().len(),
        cross_file_rule_count = analysis.cross_file_registry.rules().len(),
        resolve = analysis.resolve,
        "starting check"
    );
    analyze_parallel_collecting_resolving(
        &analysis.registry,
        files,
        config,
        analysis.collect_programs,
        analysis.resolve,
        AnalysisWorkspace {
            packages: &packages,
            environment: Some(environment),
        },
    )
}

/// Analyze an already parsed file with caller-owned cached semantic context.
/// Syntax diagnostics are supplied by the parser/cache owner, never reparsed here.
pub fn analyze_document<'a>(
    registry: &RuleRegistry,
    config: &FalconConfig,
    ctx: &'a AnalyzeContext<'a>,
    mut syntax_diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    syntax_diagnostics.extend(registry.run_all(ctx.parsed().program(), ctx));
    falcon_rules::apply_severities(&mut syntax_diagnostics, config);
    apply_fix_policies(config, &mut syntax_diagnostics);
    syntax_diagnostics
}

/// Retain a manifest using the same parsed-file representation as Dart sources.
pub fn manifest_file(path: PathBuf, source: String) -> ProjectFile {
    let tree = falcon_dart_parser::lex_tree(&source);
    ProjectFile {
        path,
        parsed: Arc::new(ParsedFile {
            source: source.into_boxed_str(),
            tree,
        }),
        has_parse_errors: false,
    }
}

/// Apply file-rule severities and append cross-file findings using existing indexes.
/// Returns whether a rule worker failed; frontends choose their own failure policy.
pub fn complete_analysis(
    environment: &AnalysisEnvironment,
    registry: &CrossFileRuleRegistry,
    files: &[ProjectFile],
    config: &FalconConfig,
    indexes: Option<&SemanticIndexes>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let mut failed = diagnostics
        .iter()
        .any(|d| d.rule == RULES_THREAD_ERROR_DIAGNOSTIC_ID);
    falcon_rules::apply_severities(diagnostics, config);
    if !registry.is_empty() {
        match cross_file_diagnostics(environment, registry, files, config, indexes) {
            Ok(cross_file) => diagnostics.extend(cross_file),
            Err(error) => {
                failed = true;
                diagnostics.extend(
                    files
                        .first()
                        .map(|file| falcon_analyze::rules_thread_diagnostic(&file.path, &error)),
                );
            }
        }
    }
    apply_fix_policies(config, diagnostics);
    failed
}

/// Filter project findings and resolve their one-based positions from retained sources.
/// Missing-path and worker-failure diagnostics are always retained.
pub fn finalize_diagnostics<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    filter: &PathFilter,
    sources: impl IntoIterator<Item = (&'a Path, &'a str)>,
) {
    diagnostics.retain(|diagnostic| {
        diagnostic.rule == UNMATCHED_PATH_DIAGNOSTIC_ID
            || diagnostic.rule == RULES_THREAD_ERROR_DIAGNOSTIC_ID
            || if Path::new(&diagnostic.file_path)
                .extension()
                .is_some_and(|ext| ext == "dart")
            {
                filter.admits(&diagnostic.file_path)
            } else {
                !filter.excludes(&diagnostic.file_path)
            }
    });
    if diagnostics.is_empty() {
        return;
    }
    let sources: HashMap<Cow<'_, str>, &str> = sources
        .into_iter()
        .map(|(path, source)| (path.to_string_lossy(), source))
        .collect();
    let mut lines: HashMap<&str, LineIndex<'_>> = HashMap::new();
    for diagnostic in diagnostics.iter_mut() {
        if let Some((path, source)) = sources.get_key_value(diagnostic.file_path.as_str()) {
            diagnostic
                .resolve_position_in(lines.entry(path).or_insert_with(|| LineIndex::new(source)));
        }
    }
    sort_diagnostics(diagnostics);
}

/// Resolve positions for findings from one admitted document without a project source map.
pub fn resolve_document_positions(diagnostics: &mut [Diagnostic], source: &str) {
    if diagnostics.is_empty() {
        return;
    }
    let lines = LineIndex::new(source);
    for diagnostic in diagnostics {
        diagnostic.resolve_position_in(&lines);
    }
}

/// Stable output order, with syntax errors preceding lints in each file.
pub fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(
                (a.rule != SYNTAX_ERROR_DIAGNOSTIC_ID).cmp(&(b.rule != SYNTAX_ERROR_DIAGNOSTIC_ID)),
            )
            .then(a.span.start.cmp(&b.span.start))
            .then(a.rule.cmp(b.rule))
            .then(a.span.end.cmp(&b.span.end))
            .then(a.message.cmp(&b.message))
    });
}
