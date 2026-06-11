//! M6 integration tests: the benchmark pipeline runs without error on the
//! jfit corpus, and optimization variants agree on diagnostic output.
//!
//! All tests skip silently when the corpus is absent (CI machines without
//! jfit checked out), mirroring the bench harness convention.

use std::path::PathBuf;

use falcon_analyze::{RuleRegistry, analyze_parallel, analyze_sequential};
use falcon_cli::{CheckOptions, collect_check, walk_files};
use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;
use falcon_rules::enabled_rules;

fn corpus_root() -> Option<PathBuf> {
    let root = std::env::var("JFIT_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/jacob/Documents/Developer/jfit"));
    let mobile_lib = root.join("apps/mobile/lib");
    mobile_lib.exists().then_some(mobile_lib)
}

fn build_registry(config: &FalconConfig) -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    for rule in enabled_rules(config) {
        registry.register(rule);
    }
    registry
}

fn sort_key(d: &Diagnostic) -> (String, usize, String) {
    (d.file_path.clone(), d.span.start, d.rule.to_string())
}

/// M6.1 exit criterion: benchmark pipeline runs without error on jfit corpus.
#[test]
fn jfit_benchmark_pipeline_runs_without_error() {
    let Some(root) = corpus_root() else {
        eprintln!("jfit corpus not found; skipping");
        return;
    };
    let files = walk_files(&[root], &[]);
    assert!(!files.is_empty(), "jfit mobile lib has no .dart files");

    let config = FalconConfig::default();
    let registry = build_registry(&config);
    let diagnostics = analyze_sequential(&registry, &files, &config);
    // The pipeline completing without panic is the contract; diagnostics may
    // legitimately be present or absent depending on corpus state.
    eprintln!(
        "analyzed {} files, {} diagnostics",
        files.len(),
        diagnostics.len()
    );
}

/// M6.2 regression guard: parallel and sequential analysis produce identical
/// diagnostics (optimizations must not change rule output).
#[test]
fn jfit_parallel_matches_sequential() {
    let Some(root) = corpus_root() else {
        eprintln!("jfit corpus not found; skipping");
        return;
    };
    let files = walk_files(&[root], &[]);
    let config = FalconConfig::default();
    let registry = build_registry(&config);

    let mut sequential = analyze_sequential(&registry, &files, &config);
    let mut parallel = analyze_parallel(&registry, &files, &config);
    sequential.sort_by_key(sort_key);
    parallel.sort_by_key(sort_key);

    assert_eq!(
        sequential.len(),
        parallel.len(),
        "parallel analysis changed diagnostic count"
    );
    for (s, p) in sequential.iter().zip(parallel.iter()) {
        assert_eq!(sort_key(s), sort_key(p), "diagnostic mismatch");
        assert_eq!(s.message, p.message, "message mismatch at {}", s.file_path);
        assert_eq!(
            s.severity, p.severity,
            "severity mismatch at {}",
            s.file_path
        );
    }
}

/// End-to-end smoke test through the public CLI entry point.
#[test]
fn jfit_collect_check_smoke() {
    let Some(root) = corpus_root() else {
        eprintln!("jfit corpus not found; skipping");
        return;
    };
    let options = CheckOptions {
        paths: vec![root],
        parallel: true,
        quiet: true,
        ..CheckOptions::default()
    };
    let output = collect_check(&options).expect("collect_check failed on jfit corpus");
    assert!(output.total_files > 0, "no files analyzed");
}
