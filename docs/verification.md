# Verification without redundant full builds

Use `makers verify-rule <id>` or `makers verify-package <package>` for iteration.
A rule check exercises only the selected CLI corpus. Package checks exercise
Clippy, all package test targets and, for packages with a library target,
doctests; they do not establish dependent package behavior. `makers verify-fast`
checks formatting/TOML, workspace Clippy and workspace tests, without release
builds or CLI corpus/doc generation.

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
