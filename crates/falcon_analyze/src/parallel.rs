use std::path::PathBuf;

use rayon::prelude::*;
use tracing::{info, info_span};

use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;

use crate::resolve::ProjectIndex;
use crate::{AnalyzeContext, ProjectFile, RuleRegistry};

/// Parse `source`, run every per-file rule, and optionally keep the parsed
/// program for the project pass. Returns the file's diagnostics plus the
/// retained [`ProjectFile`] when `collect_programs` is set.
///
/// When `resolve` is set, a degraded single-file [`ProjectIndex`] is built from
/// this file's own program and attached to the context, so resolver-dependent
/// rules can consult declaration return types (this file's declarations plus the
/// builtin table). A true cross-file index would require building one index from
/// all programs before the per-file pass; that reorder is left to the rule
/// integration phase — see `crate::resolve` and the CLI pipeline notes.
fn analyze_file(
    registry: &RuleRegistry,
    path: &PathBuf,
    source: &str,
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
) -> (Vec<Diagnostic>, Option<ProjectFile>) {
    let span = info_span!("analyze_file", file = %path.display());
    let _enter = span.enter();
    let (program, parse_errors) = parse(source);
    let local_index = resolve.then(|| ProjectIndex::from_program(&program));
    let ctx = AnalyzeContext {
        file_path: path,
        source,
        config,
        project: local_index.as_ref(),
    };
    let diagnostics = registry.run_all(&program, &ctx);
    info!(file = %path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
    let retained = collect_programs.then(|| ProjectFile {
        path: path.clone(),
        source: source.to_owned(),
        program,
        has_parse_errors: !parse_errors.is_empty(),
    });
    (diagnostics, retained)
}

/// Analyze multiple Dart files in parallel using Rayon work-stealing.
pub fn analyze_parallel(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    analyze_parallel_collecting(registry, files, config, false).0
}

/// Analyze multiple Dart files sequentially (deterministic, useful for debugging).
pub fn analyze_sequential(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    files
        .iter()
        .flat_map(|(path, source)| analyze_file(registry, path, source, config, false, false).0)
        .collect()
}

/// Analyze in parallel, additionally retaining each parsed [`ProjectFile`] when
/// `collect_programs` is set so the caller can run the project pass over them.
/// With `collect_programs = false` this behaves exactly like [`analyze_parallel`]
/// (programs are dropped after per-file analysis).
pub fn analyze_parallel_collecting(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    analyze_parallel_collecting_resolving(registry, files, config, collect_programs, false)
}

/// Sequential counterpart of [`analyze_parallel_collecting`].
pub fn analyze_sequential_collecting(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    analyze_sequential_collecting_resolving(registry, files, config, collect_programs, false)
}

/// Like [`analyze_parallel_collecting`], but with `resolve` controlling whether
/// each file's per-file rules receive a degraded single-file [`ProjectIndex`]
/// (see [`AnalyzeContext::project`]). Callers that enable resolver-dependent
/// rules pass `resolve = true`; the plain variants pass `false` for zero cost.
pub fn analyze_parallel_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    let (diagnostics, retained): (Vec<Vec<Diagnostic>>, Vec<Option<ProjectFile>>) = files
        .par_iter()
        .map(|(path, source)| {
            analyze_file(registry, path, source, config, collect_programs, resolve)
        })
        .unzip();
    (
        diagnostics.into_iter().flatten().collect(),
        retained.into_iter().flatten().collect(),
    )
}

/// Sequential counterpart of [`analyze_parallel_collecting_resolving`].
pub fn analyze_sequential_collecting_resolving(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
    collect_programs: bool,
    resolve: bool,
) -> (Vec<Diagnostic>, Vec<ProjectFile>) {
    let mut diagnostics = Vec::new();
    let mut retained = Vec::new();
    for (path, source) in files {
        let (diags, file) = analyze_file(registry, path, source, config, collect_programs, resolve);
        diagnostics.extend(diags);
        retained.extend(file);
    }
    (diagnostics, retained)
}
