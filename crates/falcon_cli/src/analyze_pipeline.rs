//! Analyze pipeline: config loading, file walking, parallel analysis, and diagnostic output.

use std::path::PathBuf;
use tracing::{info, warn};

use std::collections::HashMap;

use clap::ValueEnum;
use falcon_analyze::{
    CrossFileRuleRegistry, FileSuppressions, ProjectFile, RuleRegistry,
    analyze_parallel_collecting_resolving, analyze_sequential_collecting_resolving,
};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    ResolvedCrossFileRules, ResolvedRules, apply_severities, meta::suppression_lookup,
    resolve_cross_file_rules, resolve_rules,
};
use glob::Pattern;

use crate::file_walker::walk_files;
use crate::output;

/// Per-file rules that consult [`falcon_analyze::AnalyzeContext::project`]. When
/// any is enabled, the driver parses all files first and builds ONE cross-file
/// [`falcon_analyze::ProjectIndex`] shared across the per-file pass, so these
/// rules can reason about declaration return types project-wide. Kept here
/// (rather than a rule-trait flag) as a deliberately small, explicit seam;
/// extend this list as further resolver-dependent per-file rules are integrated.
/// (Cross-file rules such as `unnecessary-nullable` build their own index inside
/// `analyze_project` and are not listed here.)
const RESOLVER_DEPENDENT_RULES: &[&str] = &[
    "no-boolean-literal-compare",
    "avoid-ignoring-return-values",
    "unnecessary-string-interpolations",
    "prefer-is-empty",
    "prefer-is-not-empty",
    "prefer-iterable-where-type",
    "prefer-collection-literals",
    "prefer-final-fields",
];

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
    let mut registry = RuleRegistry::with_lookup(suppression_lookup);
    for rule in resolved.rules {
        registry.register(rule);
    }
    registry
}

/// Build a cross-file-rule registry from the resolved cross-file rule set.
fn build_cross_file_registry(resolved: ResolvedCrossFileRules) -> CrossFileRuleRegistry {
    let mut registry = CrossFileRuleRegistry::new();
    for rule in resolved.rules {
        registry.register(rule);
    }
    registry
}

/// Honor inline `// falcon-ignore` / `// falcon-ignore-all` suppressions for
/// cross-file-rule diagnostics, mirroring the per-file pass. Suppressions are
/// read from the diagnostic's own file (matched by path) and parsed lazily. This
/// pass only *filters*; malformed-suppression diagnostics are emitted once, by
/// the per-file pass over the same sources.
fn suppress_cross_file_diags(diags: &mut Vec<Diagnostic>, files: &[ProjectFile]) {
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
            .or_insert_with(|| FileSuppressions::parse(src, &diag.file_path, suppression_lookup));
        if sup.is_empty() {
            return true;
        }
        let line = sup.line_for_offset(diag.span.start);
        !sup.is_suppressed(diag.rule, line)
    });
}

/// Run the cross-file pass and fold its diagnostics into `diagnostics`, applying
/// inline suppressions and the same path-aware severity resolution as the
/// per-file pass.
fn run_cross_file_pass(
    registry: &CrossFileRuleRegistry,
    project_files: &[ProjectFile],
    config: &FalconConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cross_file_diags = registry.run_all(project_files, config);
    suppress_cross_file_diags(&mut cross_file_diags, project_files);
    apply_severities(&mut cross_file_diags, config);
    diagnostics.extend(cross_file_diags);
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
    // Cross-file rules run a second pass over the retained programs; only collect
    // programs when at least one is enabled (they are memory-heavy).
    let cross_file_registry = build_cross_file_registry(resolve_cross_file_rules(&config));
    let collect_programs = !cross_file_registry.is_empty();
    // Resolver seam: enable cross-file type resolution only when a per-file rule
    // that consumes it is active, so a default run pays nothing. When set, the
    // engine parses all files first and builds one shared `ProjectIndex` (every
    // file's declarations + builtins) attached to each per-file context.
    let resolve = registry
        .rules()
        .iter()
        .any(|r| RESOLVER_DEPENDENT_RULES.contains(&r.name()));
    info!(
        file_count = files.len(),
        rule_count = registry.rules().len(),
        cross_file_rule_count = cross_file_registry.rules().len(),
        resolve,
        "starting check"
    );
    let (mut diagnostics, project_files) = if options.parallel {
        analyze_parallel_collecting_resolving(&registry, &files, &config, collect_programs, resolve)
    } else {
        analyze_sequential_collecting_resolving(
            &registry,
            &files,
            &config,
            collect_programs,
            resolve,
        )
    };

    apply_severities(&mut diagnostics, &config);

    if collect_programs {
        run_cross_file_pass(
            &cross_file_registry,
            &project_files,
            &config,
            &mut diagnostics,
        );
    }

    // Resolve 1-based line/col for every diagnostic from its file's source, so
    // text and JSON output carry navigable positions rather than byte offsets.
    let sources: HashMap<String, &str> = files
        .iter()
        .map(|(p, s)| (p.to_string_lossy().into_owned(), s.as_str()))
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
