# Verification without redundant full builds

Use `makers verify-rule <id>` or `makers verify-package <package>` for iteration.
A rule check exercises only the selected CLI corpus. Package checks exercise
Clippy, all package test targets and doctests; they do not establish dependent
package behavior. `makers verify-fast` checks formatting/TOML, workspace Clippy
and workspace tests, without release builds or CLI corpus/doc generation.

The final coordinator runs `makers verify-full` (`makers A`) on settled source.
It runs lint, check, tests, the shared CLI/benchmark validation build (including
CLI corpus and `docgen --check`), and release builds, including the Zed extension.
Fast/full also run the small verification-tooling tests. The existing OSS corpus,
website and Dart launcher CI jobs remain separate obligations where applicable.
There is no blanket waiver for a red or unexecuted final gate.

## Resource budget

Verification sets these defaults only for its children; explicit environment
values override them. Benchmarks and normal Falcon execution are unaffected.

| Variable | Default | Controls |
| --- | --- | --- |
| `CARGO_BUILD_JOBS` | 2 | One Cargo build's jobs |
| `NEXTEST_TEST_THREADS` | 4 | Concurrent nextest test processes |
| `RUST_TEST_THREADS` | 4 | Threads inside libtest harnesses |
| `MBX_SCHEDULER_TESTS` | 1 | mbx admission for supported test runs |

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
