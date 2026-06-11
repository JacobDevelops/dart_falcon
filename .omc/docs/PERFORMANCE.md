# Performance Methodology & Lock (M6)

Status: **LOCKED** — future changes must keep the jfit mobile lib check under
1000ms (median wall-clock, release build, reference hardware) or justify the
regression in the PR description.

## Target

Plan M6 exit criterion: **<1000ms total** (parse + analyze + output) for the
full jfit mobile lib (`apps/mobile/lib`, 214 `.dart` files), all enabled rules.

## Reference hardware

| Component | Value |
| --- | --- |
| CPU | AMD Ryzen 7 9800X3D (8 cores / 16 threads) |
| RAM | 48 GB |
| OS | Linux x86_64 (kernel 6.8) |
| Rust | rustc 1.96.0 |

## Benchmark harness

`benches/jfit_mobile_bench.rs` (workspace root package) — Criterion benchmarks
over the full jfit mobile lib, per-stage:

| Benchmark | Measures |
| --- | --- |
| `jfit_mobile/parse` | parser alone, every file |
| `jfit_mobile/parse_analyze/sequential` | parser + all rules, single-threaded |
| `jfit_mobile/parse_analyze/parallel` | parser + all rules, Rayon work-stealing |
| `jfit_mobile/full_pipeline/sequential` | parse + analyze + sort + text output |
| `jfit_mobile/full_pipeline/parallel` | same, Rayon variant |

Corpus location comes from `$JFIT_PATH` (default
`/home/jacob/Documents/Developer/jfit`); every benchmark and test returns
early when the corpus is absent, so `cargo bench` is safe on any machine.

Run with:

```sh
cargo bench -p dart_falcon --bench jfit_mobile_bench
```

## Baseline (2026-06-11, reference hardware)

Criterion medians, 214 files, ~60 rules enabled (default config):

| Stage | Time | Throughput |
| --- | --- | --- |
| parse | 16.6 ms | 12.9 Kfiles/s |
| parse_analyze/sequential | 77.5 ms | 2.8 Kfiles/s |
| parse_analyze/parallel | 23.6 ms | 9.1 Kfiles/s |
| full_pipeline/sequential | 73.7 ms | 2.9 Kfiles/s |
| full_pipeline/parallel | 23.8 ms | 9.0 Kfiles/s |

End-to-end binary (`cargo xtask perf-lock`, median of 5 runs incl. process
spawn, file walk, I/O): **25 ms** (min 24, max 26) — **40× under budget**.

Rayon parallelism yields a ~3.1× speedup over sequential on 8 cores; the
parser accounts for ~21% of analysis time, rules for the rest. Because the
target is met with this margin, no M6.2 optimization sprint was required.

## Performance lock enforcement

```sh
cargo xtask perf-lock            # builds release falcon, 5 timed runs, asserts median <1000ms
cargo xtask perf-lock --json     # machine-readable report
cargo xtask perf-lock --skip-build --runs 10 --budget-ms 500
```

The lock measures the real binary end-to-end (`falcon check --quiet --parallel`)
including process startup, file discovery, and I/O — strictly harder than the
Criterion in-process numbers. A warm-up run primes the OS file cache first so
the lock measures the linter, not cold disk reads.

Regression guard for correctness: `tests/jfit_pipeline.rs` asserts parallel and
sequential analysis produce identical diagnostics, so parallelism/optimization
changes cannot silently alter rule output.

## Profiling

`[profile.bench] debug = true` (workspace `Cargo.toml`) keeps debug symbols in
optimized bench builds so flamegraphs resolve frames:

```sh
cargo install flamegraph        # one-time; needs perf
cargo flamegraph --bench jfit_mobile_bench -- --bench full_pipeline
```

For perf directly:

```sh
perf record --call-graph dwarf -- target/release/falcon check --parallel <jfit>/apps/mobile/lib
perf report
```

## Optimization techniques in use (M6.2)

- **File-level Rayon parallelism** (`falcon_analyze::analyze_parallel`):
  work-stealing `par_iter` over files; each file is parsed and analyzed
  independently, no shared mutable state. Chosen over per-rule parallelism per
  `.omc/docs/PARALLELISM_MODEL.md` (per-file units are plentiful: 214 files vs
  ~60 rules, and avoid cross-rule synchronization).
- **Single parse per file**: all rules run against one immutable AST
  (`registry.run_all`), never re-parsing per rule.
- **Borrowed analysis context**: `AnalyzeContext` holds `&Path`/`&str`
  references — no per-rule cloning of sources or config.
- **Deterministic post-sort instead of ordered collection**: parallel results
  are collected unordered (cheap) and sorted once before output.
