use std::path::PathBuf;

use rayon::prelude::*;
use tracing::{info, info_span};

use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;

use crate::{AnalyzeContext, RuleRegistry};

fn analyze_file(
    registry: &RuleRegistry,
    path: &PathBuf,
    source: &str,
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    let span = info_span!("analyze_file", file = %path.display());
    let _enter = span.enter();
    let (program, _parse_errors) = parse(source);
    let ctx = AnalyzeContext { file_path: path, source, config };
    let diagnostics = registry.run_all(&program, &ctx);
    info!(file = %path.display(), diagnostic_count = diagnostics.len(), "file analysis complete");
    diagnostics
}

/// Analyze multiple Dart files in parallel using Rayon work-stealing.
pub fn analyze_parallel(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    files
        .par_iter()
        .flat_map(|(path, source)| analyze_file(registry, path, source, config))
        .collect()
}

/// Analyze multiple Dart files sequentially (deterministic, useful for debugging).
pub fn analyze_sequential(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &FalconConfig,
) -> Vec<Diagnostic> {
    files
        .iter()
        .flat_map(|(path, source)| analyze_file(registry, path, source, config))
        .collect()
}
