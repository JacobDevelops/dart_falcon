//! Full-pipeline benchmarks against the jfit mobile lib.
//!
//! Stages measured:
//!   1. `parse`         — parser alone over every corpus file
//!   2. `parse_analyze` — parser + all enabled rules (sequential vs. Rayon parallel)
//!   3. `full_pipeline` — parser + analyze + diagnostic sort + text output
//!
//! Target: <1000ms total for parse + analyze + output
//! on the full jfit mobile lib, reference hardware Linux x86_64, 8+ cores.
//!
//! Set `$JFIT_PATH` to the external corpus checkout; benchmarks use its
//! `apps/mobile/lib` subtree. Benchmarks are omitted when this opt-in variable
//! is unset or the corpus is absent.
//!
//! Profiling: `[profile.bench] debug = true` is set in the workspace manifest so
//! flamegraphs resolve symbols:
//!   cargo flamegraph --bench jfit_mobile_bench -- --bench full_pipeline

use std::path::PathBuf;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use falcon_analyze::{
    Workspace, analyze_parallel_collecting_resolving, analyze_sequential_collecting_resolving,
};
use falcon_cli::{format_text, walk_project};
use falcon_config::{FalconConfig, PathFilter};
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;
use falcon_rules::build_registry;

/// Locate the jfit mobile lib corpus, or None when not present.
fn corpus_root() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("JFIT_PATH")?);
    let mobile_lib = root.join("apps/mobile/lib");
    mobile_lib.exists().then_some(mobile_lib)
}

/// Load every .dart file in the corpus via the same walker the CLI uses.
fn load_corpus() -> Option<Vec<(PathBuf, String)>> {
    let root = corpus_root()?;
    let files = walk_project(&[root], &PathFilter::default(), false)
        .expect("read benchmark corpus")
        .files;
    (!files.is_empty()).then_some(files)
}

/// Stable output order for the benchmark's diagnostics.
fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then((a.rule != "syntax-error").cmp(&(b.rule != "syntax-error")))
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
        b.iter(|| {
            analyze_sequential_collecting_resolving(
                &registry,
                &files,
                &config,
                false,
                false,
                Workspace::default(),
            )
        });
    });
    group.bench_function("parse_analyze/parallel", |b| {
        b.iter(|| {
            analyze_parallel_collecting_resolving(
                &registry,
                &files,
                &config,
                false,
                false,
                Workspace::default(),
            )
        });
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
            let (mut diagnostics, _, _) = analyze_sequential_collecting_resolving(
                &registry,
                &files,
                &config,
                false,
                false,
                Workspace::default(),
            );
            sort_diagnostics(&mut diagnostics);
            format_text(&diagnostics)
        });
    });
    group.bench_function("full_pipeline/parallel", |b| {
        b.iter(|| {
            let (mut diagnostics, _, _) = analyze_parallel_collecting_resolving(
                &registry,
                &files,
                &config,
                false,
                false,
                Workspace::default(),
            );
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
