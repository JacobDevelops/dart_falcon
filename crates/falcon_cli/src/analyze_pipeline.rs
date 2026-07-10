//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::path::PathBuf;
use tracing::{info, warn};

use std::collections::HashMap;

use clap::ValueEnum;
use falcon_analyze::{
    FileSuppressions, ProjectFile, ProjectRuleRegistry, RuleRegistry, analyze_parallel_collecting,
    analyze_sequential_collecting,
};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    ResolvedProjectRules, ResolvedRules, apply_severities, resolve_project_rules, resolve_rules,
};
use glob::Pattern;

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

/// Build a registry from the resolved rule set (enablement semantics live in
/// `falcon_rules::resolve_rules`).
fn build_registry(resolved: ResolvedRules) -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    for rule in resolved.rules {
        registry.register(rule);
    }
    registry
}

/// Build a project-rule registry from the resolved project rule set.
fn build_project_registry(resolved: ResolvedProjectRules) -> ProjectRuleRegistry {
    let mut registry = ProjectRuleRegistry::new();
    for rule in resolved.rules {
        registry.register(rule);
    }
    registry
}

/// Honor inline `// ignore:` / `// ignore_for_file:` suppressions for
/// project-rule diagnostics, mirroring the per-file pass. Suppressions are read
/// from the diagnostic's own file (matched by path) and parsed lazily.
fn suppress_project_diags(diags: &mut Vec<Diagnostic>, files: &[ProjectFile]) {
    if diags.is_empty() {
        return;
    }
    let sources: HashMap<String, &str> = files
        .iter()
        .map(|f| (f.path.to_string_lossy().into_owned(), f.source.as_str()))
        .collect();
    let mut cache: HashMap<String, FileSuppressions> = HashMap::new();
    diags.retain(|diag| {
        let Some(src) = sources.get(&diag.file_path) else {
            return true;
        };
        let sup = cache
            .entry(diag.file_path.clone())
            .or_insert_with(|| FileSuppressions::from_source(src));
        if sup.is_empty() {
            return true;
        }
        let line = sup.line_for_offset(diag.span.start);
        !sup.is_suppressed(diag.rule, line)
    });
}

/// Run the project (cross-file) pass and fold its diagnostics into `diagnostics`,
/// applying inline suppressions and the same path-aware severity resolution as
/// the per-file pass.
fn run_project_pass(
    registry: &ProjectRuleRegistry,
    project_files: &[ProjectFile],
    config: &FalconConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut project_diags = registry.run_all(project_files, config);
    suppress_project_diags(&mut project_diags, project_files);
    apply_severities(&mut project_diags, config);
    diagnostics.extend(project_diags);
}

/// Keep only files matching at least one positive include glob. A non-empty
/// `includes` list restricts the walked set; an empty one means "no filtering".
fn apply_includes(files: &mut Vec<(PathBuf, String)>, includes: &[String]) {
    if includes.is_empty() {
        return;
    }
    let compiled: Vec<Pattern> = includes
        .iter()
        .filter_map(|p| match Pattern::new(p) {
            Ok(pat) => Some(pat),
            Err(_) => {
                warn!("invalid include pattern: {}", p);
                None
            }
        })
        .collect();
    files.retain(|(path, _)| {
        let s = path.to_string_lossy();
        compiled.iter().any(|p| p.matches(&s))
    });
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
    let mut exclude_patterns = config.files.exclude_patterns();
    exclude_patterns.extend(options.exclude_patterns.iter().cloned());

    let mut files = walk_files(&options.paths, &exclude_patterns);
    apply_includes(&mut files, &config.files.include_patterns());
    if files.is_empty() {
        return Ok(CheckOutput {
            diagnostics: vec![],
            total_files: 0,
            exit_code: 0,
        });
    }

    let resolved = resolve_rules(&config);
    let registry = build_registry(resolved);
    // Project (cross-file) rules run a second pass over the retained programs;
    // only collect programs when at least one is enabled (they are memory-heavy).
    let project_registry = build_project_registry(resolve_project_rules(&config));
    let collect_programs = !project_registry.is_empty();
    info!(
        file_count = files.len(),
        rule_count = registry.rules().len(),
        project_rule_count = project_registry.rules().len(),
        "starting check"
    );
    let (mut diagnostics, project_files) = if options.parallel {
        analyze_parallel_collecting(&registry, &files, &config, collect_programs)
    } else {
        analyze_sequential_collecting(&registry, &files, &config, collect_programs)
    };

    apply_severities(&mut diagnostics, &config);

    if collect_programs {
        run_project_pass(&project_registry, &project_files, &config, &mut diagnostics);
    }

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
