# Benchmarks

These are local, whole-command measurements of release falcon and `dart analyze`, not a claim that the tools perform identical work. Both receive an explicit shared set of official Dart lint names. **`dart analyze` additionally performs full Dart type analysis and reports compiler errors, warnings, and non-lint diagnostics.** Falcon is not a replacement for that type checker.

The machine-readable results, individual wall-clock samples, exact commands, diagnostic distributions, source revisions, and dependency-resolution failures are published in [benchmarks.json](https://github.com/JacobDevelops/dart_falcon/blob/main/website/src/data/benchmarks.json). The [interactive benchmark page](https://dart-falcon.jacobdevelops.com/benchmarks) reads the same data.

## Corpora

| Corpus | Pinned revision | Selected source root | Dart files | Lines |
| --- | --- | --- | ---: | ---: |
| [dio](https://github.com/cfug/dio) | `f1bef462289b40992d625d4eb2a2f42321217573` | `dio/` | 64 | 12,787 |
| [bloc](https://github.com/felangel/bloc) | `b9be1e252ee24ffbe9f2fb629edc7624b2cfbf05` | `packages/bloc/` | 43 | 4,950 |
| [flame](https://github.com/flame-engine/flame) | `141d574d8ee0fef196c981f5ed52ddd92ef72052` | repository root | 1,771 | 194,451 |
| [flutter/packages plugins](https://github.com/flutter/packages) | `759473e08b8ba4766bf1d6356761cf9e2a900f88` | `packages/camera/`, `packages/google_maps_flutter/`, `packages/webview_flutter/` | 726 | 242,771 |

## Reproduction

From the repository root on Linux, with Rust, mise, git, GNU time (`/usr/bin/time`), gzip, GNU base64, sha256sum, and network access available:

```sh
mise install hyperfine@2.0.0 flutter@3.47.7-stable
cargo xtask bench-compare
```

Run on an otherwise idle machine without concurrent builds, tests, or other substantial workloads. `makers bench_compare` is an equivalent task alias. Run `cargo xtask bench-compare --help` for overrides. The defaults are one warmup, three measured cold-cache runs, and five measured warm-cache runs per command. `--cold-runs`, `--runs`, and `--warmups` change those counts; `--skip-build` explicitly reuses an existing release binary. `--dart` must name the real SDK executable rather than a shim; `--hyperfine` overrides hyperfine's executable.

The harness builds `target/release/falcon` with Cargo's `--locked` flag, verifies Dart 3.13.5, fetches pinned source commits into the gitignored `target/bench-corpora/` directory, and runs `flutter pub get` during setup, outside the timed section. The release executable's SHA-256 is recorded alongside its reported version. Dependency setup is repeated to refresh SDK paths even when a package configuration already exists. Existing lockfiles are enforced rather than silently upgraded. Each corpus embeds all resolved `pubspec.lock` files in `dependencyLocksGzipBase64`: base64 of a gzip-compressed JSON object mapping relative lockfile paths to their complete content. A reproduction restores these snapshots from the committed results before running `pub get --enforce-lockfile`. This preserves registry versions, integrity hashes, and resolved git revisions without shipping large uncompressed lockfile copies to the website. An incomplete checkout or a changed pin is repaired by fetching and checking out the required revision inside the disposable cache.

Subpackages whose dependencies cannot resolve are excluded from **both** tools and the reported source counts; their paths and package-manager errors are retained in `corpora[].setupFailures`. A reproduction preserves these exclusions, removes cached lockfiles absent from the recorded snapshot, and aborts if another package stops resolving or the source counts change, rather than silently shrinking the workload. A previous report without lock snapshots cannot be used to reproduce dependency resolution. Workspace members use their workspace's dependency resolution. This is not a successful-build benchmark: analyzer compilation diagnostics on the remaining source are retained and counted.

The flutter/packages workload is a sparse checkout of the camera, google_maps_flutter, and webview_flutter package families, not the entire monorepo. The included directories are recorded in the report. The complete monorepo was rejected as unsuitable for this machine; no timing or memory number from that interrupted attempt is published.

Before each tool/scenario's timing batch, one separate untimed invocation captures diagnostics and GNU time's `%M` maximum resident set. `peakRssMb` converts its KiB value into decimal megabytes; this is the largest individual process resident set reported by GNU time, not summed allocations or a claim of peak aggregate memory. All tools and corpora run sequentially. A Linux `/proc` monitor samples the complete descendant process tree every 100 ms and recursively stops and kills that tree if aggregate RSS exceeds 8 GiB or a batch exceeds 15 minutes. Commands inherit the foreground process group so terminal Ctrl-C reaches the tools directly. The guard remains active during hyperfine; no memory-monitoring wrapper is placed inside the timed command. An over-limit or failed measurement stops publication instead of being counted as a fast result.

The same monitor rejects a pass if any benchmark process has nonzero Linux `VmSwap`. Host-wide swap-in and swap-out page-counter deltas are separately recorded as `swapPagesRead` and `swapPagesWritten`; these include unrelated desktop activity and are not presented as the tools' own I/O. Already occupied but inactive system swap is not a measurement of benchmark memory.

## What is timed

Each sample starts a fresh process and measures the complete CLI invocation, including configuration, directory discovery, parsing, lint execution, and diagnostic serialization. The commands are `falcon check . --config <generated-config> --format json --exit-code 0` and `dart --suppress-analytics analyze --format=machine .`. Both operate on the same corpus directory. Dart telemetry is explicitly suppressed so a user's analytics setting and network do not add an exit-time wait. There is no mise, cargo, Flutter launcher, dependency download, or release build inside the timed command. Hyperfine 2.0.0 exports wall-clock samples; the published numbers are its mean, sample standard deviation, minimum, and maximum in milliseconds, not a best-of-run result.

All existing analysis-options files in the disposable checkout are replaced by one explicit corpus-root configuration; project-specific lint includes and settings do not influence the comparison. Falcon's recommended file and cross-file rule sets are disabled before enabling the corresponding mapped official rules. This deliberately enables conflicting style preferences too: it is a shared-rule exercise, not a recommended application configuration. No falcon-only rules are added. Hidden files/directories, dependency caches, and generated build directories (`.dart_tool`, `build`) are excluded from both tools. Source files, including tests and checked-in generated Dart, otherwise remain in scope. File and line counts refer to that source tree, not imported SDK or pub-cache dependencies that the analyzer may need to load. Before timing, Falcon's public CLI file walker must discover exactly the reported Dart-file count with the generated configuration; a mismatch stops publication.

The repository's metadata is the authoritative source of analyzer names (`RuleSource::Lints` in `crates/falcon_rules/src/meta.rs`). The older headline of 142 official ports does not describe the full current mapping; the benchmark enables all 215 current mapped names. Undefined, removed, or replaced lint-name diagnostics cause the harness to stop rather than publish a silently different rule set.

The analyzer also reports six `INCOMPATIBLE_LINT` configuration warnings because this exhaustive set includes opposing style preferences. Those warnings do not remove either enabled rule: Dart 3.13.5's [options parser](https://github.com/dart-lang/sdk/blob/3.13.5/pkg/analyzer/lib/src/analysis_options/analysis_options_parser.dart#L153-L157) passes the configuration to the [rule registry](https://github.com/dart-lang/sdk/blob/3.13.5/pkg/analyzer/lib/src/lint/registry.dart#L117-L128), which selects every explicitly enabled lint. The warnings remain in the non-lint diagnostic counts.

## Cache scenarios

- **Cold analyzer cache:** `ANALYZER_STATE_LOCATION_OVERRIDE` points at a corpus-specific directory inside `target/bench-corpora/`. Hyperfine removes only that directory before every invocation, including warmups. The machine's real `~/.dartServer` directory is never cleared. The Linux filesystem page cache is not dropped, so “cold” does **not** mean a cold disk, SDK download, or dependency installation.
- **Warm analyzer cache:** the same isolated directory is retained across warmups and measured invocations. Every invocation still starts a fresh analyzer process; no analysis-server daemon is kept alive.
- Falcon has no persistent analysis cache on this command path. Its two scenarios use the same fresh-process command. Clearing the isolated analyzer directory is also applied before cold falcon runs, outside timing, for symmetry.

The analyzer's physical filesystem implementation documents `ANALYZER_STATE_LOCATION_OVERRIDE` as its state-directory override. After the measured run, the isolated directory contained `.analysis-driver/` entries, `.plugin_manager/`, `.instrumentation/`, and `.prompts/`; the analyzer actually used the overridden location.

## Interpretation and limits

Diagnostic counts are captured in a separate untimed invocation with the identical command and configuration. The report distinguishes `lintDiagnostics` for the enabled names, `nonLintDiagnostics`, `errorDiagnostics`, and the total `diagnostics` count. Both tools' per-rule keys are normalized to lowercase official analyzer names using the metadata mapping; case-significant names in the configuration remain unchanged. Totals are not discarded on a nonzero analyzer exit. Analyzer exit codes 0–3 are expected diagnostic outcomes; unexpected failures abort publication. Falcon is forced to exit zero for lint findings; a nonzero timed or capture-process falcon exit aborts publication. Timing output and the separate machine-readable diagnostic captures remain in the cache directory for inspection.

Equal enabled names are not evidence of equal emitted diagnostics on arbitrary applications. Source exclusions, generated-code handling, suppressions, analyzer-only type checking, and rule implementations can affect output. The results below report observed differences rather than claiming corpus-wide diagnostic parity from a shared configuration. The project's focused parity fixtures and these public-project measurements answer different questions.

One verified difference is suppression syntax, not an extra rule: Falcon deliberately recognizes `falcon-ignore` directives rather than Dart's `ignore` / `ignore_for_file` comments. Bloc's `example/main.dart:1` suppresses `avoid_print` for the analyzer, while Falcon reports the unchanged `print` calls. Likewise, `lib/src/bloc.dart:83,182` suppresses `prefer_asserts_with_message`, and `lib/src/bloc_base.dart:32` suppresses `one_member_abstracts` only for the analyzer. These original comments are retained; the benchmark does not rewrite source to manufacture matching totals. This explains those observed discrepancies, not every possible difference between the two implementations.

These measurements are specific to this CPU, operating system, SDK, compiler build, and source revisions. CPU frequency, background load, filesystem caching, and dependency resolution can affect results. They are not a controlled comparison of asymptotic algorithms or an assertion that a linter can replace type checking. DCM and `custom_lint` were not installed as local executables or globally activated Dart packages on this machine, so neither is assigned a fabricated result. Immich's mobile project was considered but excluded because its exact Flutter 3.47.2 constraint does not match the required Flutter 3.47.7 / Dart 3.13.5 environment.

## Published measurements

The [benchmark page](https://dart-falcon.jacobdevelops.com/benchmarks) renders timing charts, result tables, diagnostic counts, and largest-process peak RSS directly from the committed JSON, immediately above this methodology. Those generated views, rather than hand-copied tables here, are the source of measured results. The JSON also records the measurement date, machine, tool versions, executable hash, and individual samples. `machine.cores` is the number of available logical threads, not physical CPU cores.

Published `command` fields use executable basenames and corpus-relative configuration paths for portability. The actual timed invocations resolve the SDK and release executables to direct absolute paths before starting hyperfine; no launcher overhead is timed.

Cold and warm results must be considered together. An analyzer-to-Falcon time ratio above one favors Falcon; below one favors the analyzer. Cold analyzer state is relevant to fresh CI environments, but it does not represent every CI setup: persistent analyzer caches can change the result.

### Diagnostic differences

The shared configuration is reproducible, but **corpus-wide diagnostic parity was not observed**. The page and JSON retain each tool's actual counts, including parser/compiler errors and non-lint warnings:

- Bloc's extra lint findings correspond to the original Dart-only suppressions described above: `avoid_print`, `prefer_asserts_with_message`, and `one_member_abstracts`. The analyzer also reports incompatible-rule warnings.
- Checked-in generated Pigeon sources, such as `camera_android/lib/src/messages.g.dart:7` and `google_maps_flutter_android/lib/src/messages.g.dart:7`, contain `// ignore_for_file: type=lint`. Dart honors this suppression; Falcon's distinct suppression syntax does not. Thus both traverse the same source tree without emitting the same generated-source lint workload. This is a material limitation of an enabled-name comparison, not evidence of equivalent lint execution.
- The Flutter plugin families include shared helper sources without their own package manifest. For example, `google_maps_flutter_ios_shared_code/example/integration_test/google_maps_test.dart` cannot resolve `dart:ui`, `package:flutter/material.dart`, or its example application's imports in this standalone checkout. The analyzer retains these import/type errors and their downstream diagnostics; successful dependency setup does not mean these helper files form a buildable application. The plugin workload therefore must not be presented as a clean application-build or equal-diagnostic benchmark.
- Falcon reports parser diagnostics in `camera_web/example/integration_test/helpers/mocks.mocks.dart`, starting at line 274 on the record-type cast `as ({int? audioBitrate, int? videoBitrate})`. These are retained and counted, not hidden as successful parsing.
- Dio's generated `test/mock/http_mock.mocks.dart` explicitly suppresses `unnecessary_parenthesis` for Dart. Other observed differences include Dio's `require_trailing_commas` and `prefer_final_parameters`, and Flame's `avoid_print` and `cascade_invocations`. Suppression differences are verified, but the remaining per-rule differences have not all been attributed to individual source expressions. The full distributions are published rather than treating matching rule names as proof of exact parity.
- Flame's analyzer non-lint findings include incompatible-rule and deprecated-lint warnings. No configured name was unrecognized, removed, or replaced.

These measurements compare the commands a user runs, with their actual suppression behavior and type-analysis work. They are not an isolated measurement of equal amounts of lint-engine work.
