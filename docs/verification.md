# Verification without redundant full builds

Use `makers verify-rule <id>` or `makers verify-package <package>` for iteration.
A rule check exercises only the selected CLI corpus. Package checks exercise
Clippy, all package test targets and, for packages with a library target,
doctests; they do not establish dependent package behavior. `makers verify-fast`
checks formatting/TOML, workspace Clippy and workspace tests, without release
builds or CLI corpus/doc generation. The Clippy lane also checks only the
production `falcon_rules` library with `.config/clippy-rules-fs/clippy.toml`,
denying live filesystem access and analyzer discovery helpers: rules consume
captured `AnalysisEnvironment` facts. This separate `--no-deps --lib` invocation
keeps fixture I/O in tests outside that policy without lint suppression.

The final coordinator runs `makers verify-full` (`makers A`) on settled source.
It runs lint, check, tests, the shared CLI/benchmark validation build (including
CLI corpus and `docgen --check`), and release builds, including the Zed extension.
Fast/full also run the small verification-tooling tests. The existing OSS corpus,
website and Dart launcher CI jobs remain separate obligations where applicable.
There is no blanket waiver for a red or unexecuted final gate.

## Resource budget

Verification sets these defaults only for its children; explicit environment
values override them. Benchmarks and normal Falcon execution are unaffected,
except for `MBX_CACHE_LINKS` as described below.

| Variable | Default | Controls |
| --- | --- | --- |
| `CARGO_BUILD_JOBS` | 2 | One Cargo build's jobs |
| `NEXTEST_TEST_THREADS` | 4 | Concurrent nextest test processes |
| `RUST_TEST_THREADS` | 4 | Threads inside libtest harnesses |
| `MBX_SCHEDULER_TESTS` | 1 | mbx admission for supported test runs |
| `MBX_CACHE_LINKS` | false | Linked test executables change every edit and are rarely reused; caching them doubles writes |

`mise.toml` also defaults `MBX_CACHE_LINKS` to false in its `[env]`, so any
command run in the repository through mise (`cargo`, `cargo bench`, `makers`)
skips the same linked-binary cache writes; an explicit value still wins. CI's
`mbx-setup` action sets it to true because later CI runs restore linked binaries
from main's saved snapshots.

Compilation and test defaults match the corresponding LOKE values: two compile
jobs, four nextest tests and four libtest threads. Falcon sets them in this wrapper;
LOKE sets compilation/libtest in cargo-make and nextest in its profile. Falcon's
low-level tasks still bypass these wrapper defaults. The wrapper also passes this
environment to Blaze iteration, but nextest/mbx settings do not schedule Blaze's
own compiler or test runner, and Cargo's job count is not a Blaze memory/CPU cap.

Falcon does not set `RAYON_NUM_THREADS` or `MBX_SCHEDULER_PRIORITY`: caller values
or the underlying tool defaults apply. All explicit overrides pass through,
including native forms such as nextest's `num-cpus`; each tool validates its own
settings. No global machine setting is changed.

These defaults are not memory limits or a promise of responsiveness. Concurrent
tests can each start Rayon pools or child analysis processes; no wrapper limit is
imposed on those pools now. mbx retains its shared compiler/test admission and
cache reuse, but its memory estimates are not OS-enforced caps. Account for other
workspaces and applications when selecting explicit overrides.

Use current mbx (test scheduling is supported by 1.28.0). Plain Cargo remains an
option with `CARGO_CMD=cargo`; per-command thread limits remain but mbx admission
and compiler-cache sharing do not. Doctests and directly executed corpus children
are not individually scheduled by mbx. Libtest honors its thread setting, but
this wrapper does not impose a general bound on child-process fan-out.
Nextest's existing timeout includes time spent waiting for mbx admission. Inspect
runner stderr when a test times out; a timeout is a failure, never a skipped pass.

Keep each workspace's target directory separate. Do not point independent
workspaces at the same writable `CARGO_TARGET_DIR` or clear the shared cache to
resolve contention. Cargo may serialize writes to one target; mbx safely shares
matching compiler outputs across targets and deduplicates equivalent compilations.
The verification runner adds no host queue, daemon or completed-check cache.

## Integration builds and debug information

Each package registers one integration target, `tests/it.rs`, with automatic test
discovery disabled. Add test files as modules there, keeping fixtures beside their
existing source files and shared helpers at the target's crate root. Select a
suite with `cargo test -p falcon_rules --test it corpus_tests::`, or omit the
filter to run the package's entire integration binary.

Dev/test builds retain line tables for workspace code, so backtraces still show
Falcon source locations. External dependencies omit debug information to reduce
link output and disk writes; their backtraces lose source locations, and stepping
through dependency code needs a local debug-profile override. Workspace line
tables do not provide full debugger variable/type information. Bench profiling
still uses full debug information, and release settings are unchanged.

## Evidence and cancellation

`--dry-run` shows the exact commands and budgets without running checks:

```sh
makers verify-full --dry-run
makers verify-package falcon_config --dry-run
```

Python 3.11+ and Git are required. Each command has a 30-minute deadline; override
with `--timeout <seconds>`. Failure, timeout or cancellation returns nonzero and
stops the remaining commands. Cancellation terminates only the runner's own child
process group on Linux/macOS. No unrelated agents or cache workers are cleaned up.

The runner hashes tracked and nonignored new files, including deletions, file
modes and symlink identities/content, before and after commands. Observed changes
invalidate success. Ignored build outputs are excluded; external SDKs/dependencies
and ignored local configuration are not certified by that hash. Keep them stable
as well. Hashes cannot detect edit-and-revert during execution, so verification
still requires settled writers. Archive/download users need a Git checkout for
these provenance-aware entrypoints; low-level tasks remain available.

Record the command, reported source fingerprint, actual toolchain/environment,
exit result and review scope in the change handoff. A source fingerprint is not
an approval or proof of semantic correctness. Every verification-tier invocation executes fresh
checks; prior output never suppresses their execution. The optional Blaze
iteration tiers below have a separate test-reuse contract. Do not use a personal runner's
joined/cached result to certify a changed source tree or toolchain.

## Optional Blaze iteration

`mise install blaze` installs the pinned official `indexable-inc/blaze` release.
Use these package-scoped commands between edits:

```sh
makers iterate-check falcon_syntax
makers iterate-build dart_falcon
makers iterate-test falcon_syntax
```

They require the version pinned in `mise.toml`, use separate `target-blaze/`
artifacts per workspace, and retain the wrapper's timeout/source-drift checks.
The compiler's persistent database shares unchanged code across Git worktrees.
`iterate-test` defaults to Blaze's `affected` mode: eligible prior test passes
may be reused by reachable-code/input keys. `BLAZE_TESTS=all` forces execution;
`BLAZE_TESTS=verify` checks retained passes. The summary distinguishes reused tests
from executed tests. Filesystem/environment/process-dependent tests are generally
not reusable without Blaze's experimental sandbox; Falcon does not enable it.
There is no claim that two whole test commands join into exactly one execution.

Blaze uses its own compiler/toolchain and has incomplete borrow checking. These
commands are iteration feedback, not a substitute for mbx Clippy, nextest or the
final `verify-full` gate. Do not set `CARGO_CMD=blaze`. No full-gate receipt is
cached or reused. Build-script, dependency and unsupported-code failures remain
failures; use the scoped Rust gates when the pinned Blaze cannot handle a package.

The daemon outlives a command and keeps compiler state in memory. The normal
per-user daemon is shared; do not stop it to clean up an individual task.
`BLAZE_HOME` selects a separate daemon for owned experiments. Cancellation stops
the client/check process group, not a shared daemon. `CARGO_BUILD_JOBS` and thread
budgets do not cap Blaze worker memory. Its pinned `BLAZE_MEMORY_MAX` evicts idle
workers but exempts busy/recent workers, so it is not a hard cap either. Resource
limits for experiments belong to their owned process scope, not global settings.

Personal Numen coordination, loops and review dispatch stay outside the OSS
repository. Those tools should invoke these contributor-facing commands.

## Attributing analysis time

Stage tracing is DEBUG-level and opt-in; ordinary `RUST_LOG=info` does not enable it.
The CLI default filter also disables `falcon::stages` under `--verbose`.
Capture stage-close records (including `time.busy` and `time.idle`) on stderr:

```sh
RUST_LOG='off,falcon::stages=debug' falcon check --quiet --log-format json /path/to/project 2>stages.jsonl
```

Add `falcon::detail=debug` to that filter for file paths, individual rule
preparation, and cross-file rule spans. `--verbose` also enables detail logging,
but does not enable stages unless the environment filter requests them.
Do not use instrumented runs as the uninstrumented regression baseline.

| Span | Attribution |
| --- | --- |
| `analysis` | CLI collection envelope, including workspace/configuration selection |
| `discovery` | Selected project walk and source reads |
| `parse` | One selected or companion source parse, including supported-input checks |
| `environment_capture` | Package identities, manifests/localization metadata, or external/companion acquisition at the emitting boundary |
| `load_external_libraries` | External dependency closure capture, including dependency parsing; nested within environment capture |
| `semantic_index_build`, `semantic_index_update`, `semantic_index_body_refresh` | Shared library grouping, identity, type and signature index construction/update, or a body-only refresh; LSP external-only builds carry `layer="external"` |
| `registry_preparation` | Rule preparation, including rules that return diagnostics immediately |
| `lazy_file_index` (`fact` field) | First per-file construction of function bodies, const contexts, local declarations, local writes/mutations, flow writes, or globals; cache hits emit nothing |
| `semantic_state` | Per-file shared semantic state initialization, separate from callbacks |
| `shared_structural_traversal`, `shared_semantic_traversal` | Shared walks and their rule callbacks; never charge these as individual rule totals |
| `registry_finish`, `registry_reporting` | Semantic collector completion, then diagnostic normalization/suppressions/sort |
| `cross_file_rules` | Cross-file registry execution; optional `cross_file_rule` children cover complete individual project rules |
| `reporting` | Project diagnostic filtering, source positions and ordering, not terminal serialization |
| `lsp_cross_file_pass` | LSP cross-file envelope, including cache decisions and publication assembly |
| `source_snapshot_capture` | LSP semantic/cross-file snapshot acquisition, including uncached disk/dependency parsing |

`prepare_rule` is deliberately not called `run_rule`: visitor callbacks execute
later in shared traversal, while diagnostic-only rules finish during preparation.
The per-file `analyze_file` envelope and its completion event are detail-only.
No node-level timing instrumentation is installed.

Stage records form a tree, not an additive list. Sum only sibling/leaf stages
with the same meaning; do not add preparation to its detail children, external
loading to its environment envelope, or LSP/CLI envelopes to worker totals.
Lazy file indexes are first-use children of the stage or rule callback that
requests them: globals and flow-write facts normally build under `semantic_state`,
while rules can request other facts during preparation or traversal. Parent
durations are inclusive: subtract lazy-index children to distinguish shared index
cost from preparation, semantic-state or traversal work; do not sum both.
Workers restore the originating subscriber and parent span on their own thread,
including autofix reanalysis, nested large-stack transfers and Rayon jobs.
Caveat: unparented jobs sharing a subscriber can inherit a worker parent;
inactive (`None`) jobs can inherit a scoped worker subscriber, affecting
embedders using scoped subscribers. Shipped Falcon is unaffected because it
uses a global subscriber and serializes top-level analyses.
Parallel leaf-stage busy times are aggregate worker time, not wall latency.
Shared ancestor spans can be entered concurrently: their busy/idle fields are
not a wall-clock measurement; time total wall latency separately with an
uninstrumented process timer. Whole-project semantic index build records remain
outside registry traversal and rule preparation.
Cache hits correctly omit build/update records when no rebuild/update runs.

Audit inventory: before this attribution pass, the registry had a debug
`run_rule` span and “running rule” event around preparation only; parallel
analysis had info-level file envelopes/completion events and an external-load
span with loaded/failure events; semantic index construction had three debug
count events but no stage span. Those index count events remain debug summaries,
not timing probes. The CLI pipeline had only an optional manifest warning;
project analysis had a starting-check info event plus discovery/workspace/
autofix warnings; the LSP had document-completion/config-reload debug events
and dependency/source/worker/config warnings. These diagnostic events remain
events, not stages. The only explicit `Instant`/elapsed timing in the audited
analysis files belongs to an ignored semantic-index benchmark test, not the
production path. External closure parsing and LSP source acquisition remain
explicit inclusive stages rather than pretending their I/O and parse costs
are separately measured.

## Portable full-analysis measurements

`cargo xtask bench-compare --full-analysis --runs 3 --output target/full-analysis.json`
uses `xtask/full-analysis-cases.json` and the existing `xtask/e2e-corpus.json`
pins/checkouts, without requiring Dart, Flutter, DCM or DCL. Linux `/proc`,
Hyperfine 2 (the mise pin), jj, Git, GNU time and GNU coreutils are required. The cases cover
LocalSend's Flutter app, the pure-Dart bloc package, and bloc's multi-package
workspace. These are full Falcon checks, not a Dart analyzer comparison.

Every declared file/cross-file rule is explicitly enabled at warning severity;
the complete generated configuration is in each JSON result. Resolution and
cross-file execution are required. Dependencies are deliberately not downloaded:
SDK discovery is disabled, every case must load zero external libraries, and
`git status --porcelain --ignored` must be empty for each pinned checkout.
The default portable lane does not claim resolved Flutter/package API coverage.
The dependency-resolved lane is selected separately below.

Untimed JSON tracing before and after each timing batch must prove the pinned
file count, enabled registries, every file's completed analysis/preparation,
semantic index construction and every cross-file rule's completed execution.
Work counters and per-rule finding counts must also match the host-independent
`xtask/full-analysis-expected.json`; a no-op cross-file dispatcher or lost
resolution findings cannot pass by emitting the same spans. Failures include
actual/expected counters and per-rule deltas in the error and report's
`workDifferences`, so review them before using `--bless`. Re-bless after rule
changes when you next run the lane, only after reviewing intentional work changes.
Blessing one suite preserves the other suite's expectations.
Warnings, parser/internal/configuration failures, missing targets, dirty checkouts,
nonzero exits, missing spans and changed findings fail with case-named errors.
Every case is attempted; failures are written to the report and return nonzero.

Hyperfine measures fresh, uninstrumented processes with uncached analysis and
retained OS page cache. JSON records all samples, median/range, toolchain, host,
binary hash/source revision and working-change emptiness (`@` in jj, or Git HEAD
and working-tree status outside a jj repo), pins, configuration, SDK state,
four analysis threads, instrumented preflight peak RSS (GNU time, decimal MB)
and failures.
Timed runs include full JSON finding rendering, not just analysis: large finding
sets make rendering a material share of this end-to-end measurement.
The existing 8 GiB process-tree RSS, no-process-swap and 15-minute batch guards
apply. `--skip-build` reuses the release binary in Cargo metadata's target directory; `--hyperfine` selects
its executable. Other comparative-only flags are rejected, not silently ignored.
Host-specific timings are written only to `--output`, never a checked-in baseline.
The performance policy below applies to these measurements; comparative acceptance
against Dart and regression against Falcon remain separate gates.

### Resolved workloads and analyzer comparison

The three portable cases remain the fast self-regression suite. Use
`cargo xtask bench-compare --setup-resolved` once to prepare the separately
selected dependency-resolved suite, then run
`cargo xtask bench-compare --full-analysis --resolved --runs 10 --output target/resolved.json`.
Setup acquires pinned SDKs and corpus revisions and resolves locked dependencies;
none of that setup belongs to the timed region. Missing package configuration,
changed locks, zero external libraries, incomplete-analysis warnings and changed
expected work fail the resolved lane. `--bless` updates per-rule findings and work
counters in the same expected-work file, not performance thresholds.

Setup uses mise's versioned SDK cache outside this repository: Dart `3.13.6`
and Flutter `3.47.7-stable` (bundled Dart `3.13.5`). The setup command installs
those exact tool versions rather than relying on the workspace's floating Dart
alias. Corpus revisions remain pinned in `xtask/e2e-corpus.json`. The resolved
cases are Bloc's `packages/bloc` (43 Dart files, 56 configured packages) and
Wonderous (192 Dart files after setup, 134 configured packages). Dependency snapshots are
committed under `xtask/bench-locks/`: Bloc reuses the published benchmark lock;
Wonderous's upstream lock was normalized with the pinned Flutter SDK. Setup
runs the pinned SDK's `dart pub get --enforce-lockfile`, with `FLUTTER_ROOT` set
for Flutter so package resolution does not migrate the pinned project's sources,
then stamps the package configuration and SDK identity. Measurement requires
the exact embedded lock bytes, package names/counts and existing package roots,
plus blessed nonzero resolved-library counts; it never repairs missing setup.
Setup rejects modified tracked, untracked and unexpected ignored checkout contents
except its pinned lock override and required pub-generated metadata. The resolved
Falcon config excludes generated/hidden inputs using the shared benchmark exclusions,
including `.dart_tool/**`; later Dart/YAML source changes also fail validation.

Falcon **cold** means a fresh process with no persistent analysis/result cache.
The instrumented preflight and discarded warmup populate the OS page cache; this
is not a cold-filesystem measurement. Falcon has no persistent warm-analysis mode,
so a separate Falcon warm number would measure the same process/cache contract.
All samples include rendering the complete JSON findings.

The analyzer is a separate acceptance criterion and an explicit measurement mode:
`cargo xtask bench-compare --full-analysis --resolved --analyzer-comparison --runs 10 --output target/analyzer-resolved.json`.
This first measures Falcon on the same selected cases, then measures Dart and
records both tools in the report. It does not establish identical rules or work
between the tools. Portable analyzer comparison remains informational. The Falcon
self-regression lane never
invokes Dart. Analyzer **cold** starts `dart analyze` in a fresh process with an
empty private `ANALYZER_STATE_LOCATION_OVERRIDE`; it discards no timed samples.
Analyzer **warm** starts the same command in another fresh process after untimed
analyses have populated and stabilized that private analysis-server cache's file
count. SDK/package setup and cache deletion/prepopulation are outside timing;
OS page cache is retained in both modes. Reports record the
exact commands, versions, raw samples, median, MAD, IQR and coefficient of
variation (CV, a ratio rather than a percentage).

Each analyzer invocation is exactly
`ANALYZER_STATE_LOCATION_OVERRIDE=<private-state> <pinned-sdk>/bin/dart --suppress-analytics analyze --format=json .`
from the selected project directory. Before each cold sample the runner removes
only its own private state directory. Before warm samples it runs that command
at least `max(1, --warmups)` times untimed, stopping only after two consecutive
nonempty cache-file counts agree. The cap is that minimum plus ten invocations;
failure to settle fails the mode. Every warm sample must retain that count before
and after execution; count drift fails, rather than publishing unstable timings.
Reports expose the population history, actual warmup count and settled baseline.
Stable counts do not prove unchanged bytes or a fully populated byte store.
This is a persistent-cache comparison, not a long-lived in-memory analysis server.
Resolved analyzer cases reject ERROR diagnostics, not just crashes; portable
SDK-free cases intentionally retain their unresolved findings. See the pinned SDK's
[state-root implementation](https://github.com/dart-lang/sdk/blob/3.13.6/pkg/analyzer/lib/file_system/physical_file_system.dart)
and [analysis-server byte store](https://github.com/dart-lang/sdk/blob/3.13.6/pkg/analysis_server/lib/src/analysis_server.dart).
Hyperfine 2 measures child wall time separately from resource-guard polling and
validates every analyzer sample's exit status and structured findings.

### Performance acceptance policy

The three portable cases (LocalSend, bloc and bloc workspace) are the fast
self-regression suite. Resolved Bloc and Wonderous are the representative
dependency-resolved suite. For acceptance, use ten samples per case on a quiet
host, record its load and competing heavy work, and retain all samples. The
four-thread, 8 GiB process-tree RSS, no-process-swap and 15-minute guards above
remain in force. Peak RSS is reported per case from the instrumented preflight,
not as a timed-sample peak.

Run each suite once to establish a Falcon baseline, then again with
`--baseline <report.json>`. For example:

```sh
cargo xtask bench-compare --full-analysis --runs 10 --output target/portable-baseline.json
cargo xtask bench-compare --full-analysis --runs 10 --baseline target/portable-baseline.json --output target/portable-candidate.json
```

Add `--resolved` to both commands for the resolved suite. The baseline must be a
successful full-analysis report from the same host, with matching corpus pins,
configuration, completed work, dependency setup, threads, runs, warmups and
cache/resource conditions. The full-analysis report alone includes the hostname.
Baseline metadata, exact unique case ids and resolved SDK/setup compatibility are
checked before timing. Baselines require at least ten runs. The runner recomputes
medians and unscaled MAD from raw samples rather than trusting stored summaries.
Each case fails if its new median exceeds
`baseline median × (1 + max(3 × baseline MAD / baseline median, 0.03))`.
Equality passes. Console and JSON report the medians, delta and tolerance.
Host-specific baseline JSON stays outside version control.

On the resolved suite, `--analyzer-comparison` fails unless each Falcon cold
median is at most its Dart analyzer cold median; it reports the Falcon/analyzer
ratio (passing means ≤ 1). The cold/warm definitions above are unchanged.
Uncached Falcon beating warm Dart is labelled an informational stretch goal and
never gates release. LocalSend is self-regression only, not a comparative release
case. The v1 checklist remains open until both gates pass on current main with
recorded evidence; selecting this policy alone does not certify performance.


## Architecture and readiness

`makers architecture` checks the declared local production/build dependency graph
without compiling. Fast/full gates run it, and the CI lint job runs the same command.
The separate CI tooling job exercises the Python runner and architecture-check tests.
See [the ownership contract](architecture.md) for scope and deliberate exclusions.
Local fast includes Zed formatting but omits Zed Clippy/build and release/CLI/docgen
stages; full includes them.
CI splits lint, tests and validation and additionally runs a pinned fast OSS corpus;
Dart and website jobs are path-selected. It is not identical to local full, which
also builds releases. Changes to `scripts/` select Rust CI so policy/check changes
cannot pass on Python tests alone. The [v1 checklist](v1-checklist.md) records
additional release evidence; a green local gate is not release approval.
