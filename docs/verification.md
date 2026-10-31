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
| `NEXTEST_TEST_THREADS` | 2 | Concurrent nextest test processes |
| `RUST_TEST_THREADS` | 2 | Threads inside libtest harnesses |
| `RAYON_NUM_THREADS` | 2 | Rayon pools in tests, corpus runner and child Falcon processes |
| `MBX_SCHEDULER_TESTS` | 1 | mbx admission for supported test runs |
| `MBX_SCHEDULER_PRIORITY` | low | Yield at admission to normal-priority mbx work |

These are conservative local starting points, not memory limits or a promise of
responsiveness. The limits multiply: two corpus workers may each start a Falcon
process with two analysis workers. mbx uses its existing shared compiler/test
CPU and memory admission; cache hits and independent work can proceed. Its memory
estimates are not OS-enforced caps, and other applications consume resources too.
Check overrides before increasing the budget across several workspaces.

Use current mbx (test scheduling is supported by 1.28.0). Plain Cargo remains an
option with `CARGO_CMD=cargo`; per-command thread limits remain but mbx admission
and compiler-cache sharing do not. Doctests and directly executed corpus children
are not individually scheduled by mbx; their fan-out is limited by this wrapper.
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
an approval or proof of semantic correctness. Every invocation executes fresh
checks; prior output never suppresses execution. Do not use a personal runner's
joined/cached result to certify a changed source tree or toolchain.

Personal Numen coordination, loops and review dispatch stay outside the OSS
repository. Those tools should invoke these contributor-facing commands. Blaze
may be useful as an optional edit-loop experiment, but its distinct compiler and
toolchain do not replace the Cargo/mbx correctness gates; it is not required here.
