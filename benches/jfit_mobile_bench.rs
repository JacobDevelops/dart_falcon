//! M6.1 benchmark harness: full-pipeline benchmarks against the jfit mobile lib.
//!
//! Stages measured (plan M6.1):
//!   1. `parse`         — parser alone over every corpus file
//!   2. `parse_analyze` — parser + all enabled rules (sequential vs. Rayon parallel)
//!   3. `full_pipeline` — parser + analyze + diagnostic sort + text output
//!
//! Target (plan M6 exit criterion): <1000ms total for parse + analyze + output
//! on the full jfit mobile lib, reference hardware Linux x86_64, 8+ cores.
//!
//! Corpus location: `$JFIT_PATH` (default `/home/jacob/Documents/Developer/jfit`),
//! benchmarked subtree `apps/mobile/lib`. All benchmarks return early without
//! error when the corpus is absent so `cargo bench` works on any machine.
//!
//! Profiling: `[profile.bench] debug = true` is set in the workspace manifest so
//! flamegraphs resolve symbols:
//!   cargo flamegraph --bench jfit_mobile_bench -- --bench full_pipeline

use std::path::PathBuf;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use falcon_analyze::{RuleRegistry, analyze_parallel, analyze_sequential};
use falcon_cli::{format_text, walk_files};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;
use falcon_rules::enabled_rules;

/// Locate the jfit mobile lib corpus, or None when not present.
fn corpus_root() -> Option<PathBuf> {
    let root = std::env::var("JFIT_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/jacob/Documents/Developer/jfit"));
    let mobile_lib = root.join("apps/mobile/lib");
    mobile_lib.exists().then_some(mobile_lib)
}

/// Load every .dart file in the corpus via the same walker the CLI uses.
fn load_corpus() -> Option<Vec<(PathBuf, String)>> {
    let root = corpus_root()?;
    let files = walk_files(&[root], &[]);
    (!files.is_empty()).then_some(files)
}

/// Build the rule registry exactly as the CLI does for a default config.
fn build_registry(config: &FalconConfig) -> RuleRegistry {
    let mut registry = RuleRegistry::new();
    for rule in enabled_rules(config) {
        registry.register(rule);
    }
    registry
}

/// Deterministic sort applied by the CLI before output (analyze_pipeline.rs).
fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.span.start.cmp(&b.span.start))
            .then(a.rule.cmp(b.rule))
    });
}

/// Stage 1: parser alone over the full corpus.
fn bench_parse(c: &mut Criterion) {
    let Some(files) = load_corpus() else { return };
    let mut group = c.benchmark_group("jfit_mobile");
    group.sample_size(10);
    group.throughput(Throughput::Elements(files.len() as u64));
    group.bench_function("parse", |b| {
        b.iter(|| {
            for (_, src) in &files {
                let _ = parse(src);
            }
        });
    });
    group.finish();
}

/// Stage 2: parser + analyze, single-threaded vs. Rayon parallel.
fn bench_parse_analyze(c: &mut Criterion) {
    let Some(files) = load_corpus() else { return };
    let config = FalconConfig::default();
    let registry = build_registry(&config);

    let mut group = c.benchmark_group("jfit_mobile");
    group.sample_size(10);
    group.throughput(Throughput::Elements(files.len() as u64));
    group.bench_function("parse_analyze/sequential", |b| {
        b.iter(|| analyze_sequential(&registry, &files, &config));
    });
    group.bench_function("parse_analyze/parallel", |b| {
        b.iter(|| analyze_parallel(&registry, &files, &config));
    });
    group.finish();
}

/// Stage 3: parser + analyze + sort + text output — the full check pipeline
/// (minus file I/O, which is measured once at load time, not per iteration).
fn bench_full_pipeline(c: &mut Criterion) {
    let Some(files) = load_corpus() else { return };
    let config = FalconConfig::default();
    let registry = build_registry(&config);

    let mut group = c.benchmark_group("jfit_mobile");
    group.sample_size(10);
    group.throughput(Throughput::Elements(files.len() as u64));
    group.bench_function("full_pipeline/sequential", |b| {
        b.iter(|| {
            let mut diagnostics = analyze_sequential(&registry, &files, &config);
            sort_diagnostics(&mut diagnostics);
            format_text(&diagnostics)
        });
    });
    group.bench_function("full_pipeline/parallel", |b| {
        b.iter(|| {
            let mut diagnostics = analyze_parallel(&registry, &files, &config);
            sort_diagnostics(&mut diagnostics);
            format_text(&diagnostics)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_parse_analyze,
    bench_full_pipeline
);
criterion_main!(benches);
