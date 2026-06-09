use std::path::PathBuf;

use rayon::prelude::*;
use tracing::{info, info_span};

use jdlint_config::JdlintConfig;
use jdlint_dart_parser::parse;
use jdlint_diagnostics::Diagnostic;

use crate::{AnalyzeContext, RuleRegistry};

/// Analyze multiple Dart files in parallel using Rayon.
///
/// Each file is a separate work unit: parsed independently, analyzed against
/// all rules, and diagnostics are collected. No shared mutable state.
pub fn analyze_parallel(
    registry: &RuleRegistry,
    files: &[(PathBuf, String)],
    config: &JdlintConfig,
) -> Vec<Diagnostic> {
    files
        .par_iter()
        .flat_map(|(path, source)| {
            let span = info_span!("analyze_file", file = %path.display());
            let _enter = span.enter();

            let (program, _parse_errors) = parse(source);
            let ctx = AnalyzeContext {
                file_path: path,
                source,
                config,
            };
            let diagnostics = registry.run_all(&program, &ctx);
            info!(
                file = %path.display(),
                diagnostic_count = diagnostics.len(),
                "file analysis complete"
            );
            diagnostics
        })
        .collect()
}
