# v1 readiness and cleanup

Canonical backlog for release preparation; no second cleanup tracker is needed.
The planned rule checklist was completed in `da0eb6ed` (remaining eight cross-file
rules). That revision already contains the earlier rule work and the bounded
metadata/Blaze foundation; no cherry-pick reconciliation is needed. Registration,
schema, corpus and docgen checks establish declared coverage, not complete Dart
analyzer parity or v1 readiness. The old rule-only checklist was removed at that
revision; this restores its canonical path for the remaining engineering work.

Use [the architecture contract](architecture.md) and [verification](verification.md).
Select one bounded outcome, revalidate its evidence on current main, record its
acceptance and execute/review it. Each completed item needs a commit and actual
check evidence. Keep blocked decisions open; a document or passing subset cannot
mark a behavior change complete. Paths below identify current evidence, not an
instruction to preserve the existing packaging.

## Foundation established

- [x] F1 — Explicit Falcon domain ownership/design/review guidance; Cargo local
  production/build dependency regression gate in fast/full verification and CI.
  The gate does not inspect source-level responsibilities or standalone Zed.
- [x] F2 — Scoped verification and pinned optional Blaze iteration; final mbx gate
  retained. `scripts/test_verify.py` covers source drift, failure, cancellation,
  budgets and tier plans; optional Blaze reuse is never a final pass receipt.
- [x] F3 — Bounded metadata ingestion and raw fingerprints, including LSP config
  stamps. `falcon_config/src/metadata.rs` tests limit/growth/UTF-8/recovery;
  `falcon_lsp/src/state.rs::config_stamp_bounds_tests` covers oversized and
  same-size replacement. Existing sparse CLI/LSP oversized regressions remain.

These entries describe implemented foundation, not acceptance of the open items.

## Mandatory v1 blockers, in suggested execution order

### V1-1 — Make analysis completeness explicit

- [ ] Agree and implement a shared completeness contract. The required-project-source
  first slice uses `falcon_analyze::SourceAcquisition` to distinguish acquired text,
  oversized sources and failed reads retaining their I/O error kind.
  CLI collection aborts on any required source read failure, naming the path and
  reason; LSP deliberately stays live but marks collection incomplete and suppresses
  all cross-file results. Companion failures retain partial semantic snapshots for per-file rules.
  Optional metadata remains separate: manifest failures, including oversized
  manifests, warn and skip rather than rejecting the project (bounded in V1-3a).
  `required_source_acquisition_retains_read_outcomes` covers acquired text,
  disappearance after discovery, invalid UTF-8, oversize and other I/O failure.
  `failed_required_disk_reads_are_incomplete_but_optional_reads_are_not` checks
  LSP completeness and readable-source recovery without permission-bit assumptions.
  CLI/LSP fixture `required_source_failure_blocks_partial_projects_and_recovers`
  checks invalid-UTF-8 CLI error/nonzero exit, clearing stale `unused-files`
  publication while incomplete, and recovery to an exported orphan with no warning
  (distinct from the pre-failure unreachable-orphan warning), then requires a new
  absence warning from both frontends to prove recovery restored complete analysis.
  Companion acquisition now shares the same lowest-owner outcome in
  `falcon_analyze::source` (avoiding a reverse dependency on `falcon_project`).
  Resolving analysis, LSP semantic companions, project autofix and CLI
  `FixExternals::load` reject unreadable project companions before using partial facts.
  Required companions are existing sources inside an analyzed package's pubspec
  directory or its discovered pub/Melos workspace; loose sources without a pubspec
  use their canonical containing directory. CLI current directory, VCS/config
  roots and LSP workspace folders never independently expand this boundary.
  LSP scope includes every discovered project source as well as open documents,
  so packages without open buffers count. Local imports and parts within that
  scope are required; outside path dependencies, SDK and pub-cache sources remain
  optional and warn. Project autofix also discovers enclosing pub/Melos workspaces.
  Missing, directory and not-a-directory directive targets remain silently absent;
  disappearance after required-source discovery retains the earlier V1-1a contract.
  External-source completeness remains open. Deterministic companion tests cover
  invalid UTF-8 and sparse oversized files, CLI current-directory/path-spelling
  invariance, errors/nonzero exits and actual `unnecessary-new` write controls,
  direct autofix and LSP fix-all refusal, and LSP per-file semantic retention/cross-file
  suppression. Recovery requires fresh
  `unused-files` diagnostics, including removal of a closed owner's directive on disk;
  repeated edits while unreadable do not reload the external layer.
  Discovery now retains typed `DiscoveryFailure::Metadata` and `Traversal`
  outcomes with paths and underlying errors, including walked Dart symlink target
  failures. CLI aborts on required paths; LSP caches partial paths separately from
  completeness, suppresses cross-file results and logs each failure once per
  unresolved episode. Failed paths are revalidated in both watcher modes without
  rewalking or rebuilding while the same error persists, including external targets.
  Explicit missing inputs remain unmatched paths; midwalk disappearance is absent.
  Excluded directories still prune traversal. Unreadable directories that cannot
  contain positively included files warn and skip, including under package configs.
  Declared workspace member globs preserve potentially required sources even before
  inaccessible member configurations can be loaded; loaded member filters still win.
  Deterministic public-API symlink-loop tests cover metadata failures, CLI exit,
  exclusions, dangling targets and readable recovery. LSP tests assert stable walk
  and semantic-build counts across requests/polls, then removal/recreation recovery,
  canonicalized watch events and dynamic-watch recovery without an external event.
  Permission-guarded tests cover traversal failures, include scope, exclusion and
  CLI/LSP stale-result clearing followed by independently expected recovery results.
  They print a skip when permissions are bypassed. WalkDir I/O failures still need
  permission guards: no existing public injectable traversal-error seam was found.
  Optional member-manifest and dependency-watch discovery failures retain warnings.
  The editor now publishes non-rule `incomplete-analysis` warnings for required
  source and discovery failures through the normal open-document publication path.
  Referencing import/export/part URI ranges are preferred, then a failing open file
  at line zero, otherwise affected open documents at line zero (one per path/document).
  One warning notification is sent per incomplete episode, separately from config
  warnings; successful recovery clears diagnostics and re-arms the notification.
  Protocol tests cover discovered invalid-UTF-8 sources, real Falcon suppression
  syntax, repeated edits, independently expected cross-file recovery, re-breaks,
  closing documents, recovery/re-break without open documents, config disabling,
  and a disabled cross-file configuration with semantic rules. State tests cover
  required companion acquisition (invalid UTF-8 and oversized) at directive ranges,
  rejected open documents at line zero, and deterministic symlink-loop discovery
  anchors and episodes. Optional/external metadata and unsupported/failed analysis
  still need the broader completeness contract, including worker-failure acceptance
  beyond rejected open documents. V1-1 remains unchecked; these required-source
  slices do not certify the overall acceptance below.
- Acceptance: unreadable, disappearing, invalid-UTF-8 and oversized sources plus
  worker failure cannot yield an unqualified clean complete-project result;
  recovery yields current diagnostics, not stale results. Assert exit/publication
  behavior and cross-file eligibility with independent expected outcomes.

### V1-2 — Enforce snapshot and plan invariants at their owners

- [x] Encapsulate `ParsedFile.source/tree` and `ProjectFile` relationships in
  `crates/falcon_analyze/src/cross_file.rs`; migrate real callers through validated
  construction and borrowed accessors. Test retained nodes and source replacement.
  Private snapshot fields and source-owning constructors prevent foreign
  source/tree pairs. A closed parse outcome distinguishes recovered syntax from
  rejected sources; `ProjectFile` construction and replacement refuse rejected
  snapshots. Rejected LSP snapshots retain matching trees without retrying the
  failed lexer, and cannot enter rule analysis or semantic indexing.
  `retained_node_owns_its_source_and_arena` and
  `retained_node_keeps_original_snapshot_after_source_replacement` cover retained
  nodes; `parser_worker_panics_publish_worker_diagnostics_not_syntax_errors` covers
  failed-worker snapshot coherence, rule exclusion and valid-edit recovery.
  `package_refresh_worker_failure_skips_rules_and_retains_previous_snapshot`
  covers rejection during package-version refresh.
  `rejected_snapshots_cannot_enter_a_project`,
  `rejected_body_refresh_preserves_previous_facts` and
  `rejected_signature_refresh_preserves_previous_body_facts` cover admission and
  refresh boundaries. `unparsed_tree_preserves_source_without_lexing` covers the
  opaque fallback. `numen run -- makers verify-package <pkg>` passed for
  `falcon_syntax` (85 tests), `falcon_analyze` (679 tests, 14 doctests),
  `falcon_rules` (1138 tests), `falcon_project` (78 tests), `falcon_cli` (200 tests)
  and `falcon_lsp` (230 tests). The root `dart_falcon` package passed clippy and all
  76 tests; its package wrapper then failed at `--doc` because it has no library
  target.
- [x] Encapsulate `ProjectAnalysis` registries and derived flags in
  `crates/falcon_project/src/lib.rs`. Test structural, semantic and cross-file
  combinations; no caller can independently contradict resolution/collection needs.
  Private fields and read-only accessors keep rule requirements together; the
  named autofix snapshot intent cannot disable collection needed by cross-file
  rules. A test-only semantic autofix constructor preserves custom rules' cache
  requirements. Ten plan tests cover structural, semantic and both cross-file
  modes, mixed configurations and autofix intents; three compile-fail doctests
  reject independent flag writes and mutable registry access.
  `numen run -- makers verify-package <pkg>` passed for `falcon_project`
  (88 tests, 4 doctests), `falcon_cli` (200 tests) and `falcon_lsp` (230 tests).
  No changeset: diagnostics, autofix output and frontend behavior are unchanged.
  Boundary: the primary LSP root still takes ownership of the registries and
  aggregates flags separately through `SemanticFlags`; its tests deliberately
  exercise independent file/cross-file indexing. Closing that LSP-state gap
  belongs with the R1 capability/Deref-chain cleanup, not this plan API migration.
- Acceptance: invalid relationships cannot be constructed through supported APIs;
  existing syntax/visitor/corpus and CLI/LSP parity tests remain unchanged in meaning.
  Do these as separate coherent migrations, not a new generic planning framework.

### V1-3 — Own project facts and frontend parity

- [ ] Move rule-side metadata acquisition behind a shared bounded project snapshot.
  V1-3a migrates `secure_pubspec_urls.rs`, the smallest consumer needing only
  manifest text and YAML, to `falcon_analyze::PackageMetadata::snapshot`.
  Its closed `ManifestSnapshot` captures bounded optional metadata once per
  environment: Missing is silent and emits nothing; Malformed (including YAML
  beyond serde's recursion limit) emits no YAML-dependent snapshot-consumer diagnostic and warns
  with the path; Inaccessible (I/O, invalid UTF-8 or the metadata size limit)
  warns with path and reason and skips YAML-dependent snapshot consumers for that
  package, not unrelated rules. Shared discovery emits these warnings
  whichever rules are enabled, including discovered manifests without Dart files.
  Valid snapshots retain the exact source and parsed YAML, preserving diagnostics;
  classification and fallback-name recovery ignore a leading UTF-8 BOM without
  changing source offsets. Exact sorting and package-name diagnostics cover BOM
  manifests, which previously lost parsed facts and package identity.
  Changed metadata is captured per CLI run; the LSP refreshes captured disk facts
  on save or relevant watcher events; unsaved buffers are not facts. V1-3d adds
  editor publication and invalidation for these captured facts (see below).
  Warning deduplication lasts until invalidation, so a later failure episode warns again.
  CLI subprocess and CLI/LSP parity recovery tests cover malformed YAML, invalid
  UTF-8 and sparse oversized manifests, exact recovered diagnostic identity,
  range, severity and message, and saved changes clearing stale diagnostics.
  V1-3b also migrates `sort_pub_dependencies`, `package_names`,
  `depend_on_referenced_packages`, `undeclared_dependencies`, analyzed-package
  manifest reads in `unused_dependencies` / `unused_dev_dependencies`, and
  `banned_imports` identity reads. `lint/support/package_manifests` now extracts
  facts from snapshots; its filesystem reader and production text parser are
  removed. `manifest_consumers_use_captured_facts_without_reopening_the_manifest`
  proves the migration by deleting the manifest after capture and comparing exact
  diagnostics. Dependency-section CLI/LSP recovery tests cover diagnostics and
  warning episodes, but already pass before this migration.
  Identity-only import boundaries still use immutable fallback `metadata.identity`.
  A malformed manifest with a recoverable `name` preserves own-package path denies;
  Inaccessible metadata has no name, so only raw-URI and relative-path matching
  remain available, not own-package path matching. Apart from BOM name recovery,
  this preserves existing behavior.
  V1-3c migrates `flutter_assets` (`unused-assets` / `missing-assets`) to the same
  captured manifest bytes and YAML, including BOM-first `flutter:` keys with exact
  original-byte diagnostic ranges for LF and CRLF sources. Deleting a manifest after
  capture leaves both rules' diagnostics unchanged; valid-manifest corpus diagnostics
  remain identical. Duplicate YAML keys classify the snapshot as Malformed: both
  asset rules skip it and discovery warns once.
  Remaining asset IO is specifically `unused-assets`' direct `read_dir` expansion
  of declared directories and `metadata(...).is_file()` checks on their children,
  not globbing or existence checks for individual declarations. `missing-assets`
  checks declarations, not disk existence. These queries stay rule-side and uncached:
  the CLI evaluates them once per run, and every subsequent LSP evaluation reads
  current directory contents. Moving them into the manifest snapshot would add a
  second invalidation lifecycle without making publication coherent.
  Missing/non-directory paths are silent and exempt; dangling symlinks skip only
  that entry. Other directory/entry IO errors and non-UTF-8 file names exempt the
  directory and warn with path and reason through the environment warning channel,
  deduplicated by declared directory until a successful directory read or discovery
  reset. A successful directory read re-arms that directory's warning, so a subsequent
  failure warns again without an environment reset. CLI subprocess tests cover the
  warning channel; the LSP test covers break → fix (diagnostic returns) → break,
  including one new warning in the second episode.
  V1-3e captures localization settings, template text and sibling ARB locales in the
  analysis environment only when unused-l10n is enabled, using the shared bounded
  loader; malformed or inaccessible inputs warn once per failure episode and skip
  unused-l10n for that package. Missing inputs, including non-directory ancestors,
  are silent. Cached declared l10n.yaml/ARB create, change and delete events refresh
  only affected packages and republish tracked diagnostics without rebuilding
  semantics or walking the workspace; directory events still refresh Dart membership.
  Changing configured roots updates external watcher registration. Unrelated ARBs
  and disabled rules do not admit localization events or parse localization files.
  Unsaved metadata buffers remain excluded from facts. Deleted-after-capture,
  warning/recovery, CLI/LSP parity and BOM/CRLF/supplementary UTF-16 range tests
  cover this boundary. Scratch before/after comparison preserves the 10 existing
  localization corpus findings; BOM-prefixed templates intentionally gain findings
  that main previously skipped. Original bytes, including the BOM, anchor CLI spans
  and report fingerprints; only editor positions exclude the BOM. Template paths
  are lexically normalized, retaining symlink client paths. Parent-directory
  deletion clears captured template sources and diagnostics.
  V1-3f's resolved-dependency probes in `unused_dependencies` / `unused_dev_dependencies`
  now consume bounded generation-owned raw documents, existence and directory facts;
  rule policy remains in the rules. Dependency manifests reuse the `PackageMetadata`
  snapshot owner. Missing manifests preserve the previous behavior (no resource/tool
  exemption; absent tool directories remain exempt), while malformed/inaccessible
  inputs warn and conservatively skip only the probe that consumes them. CLI captures
  once per run; LSP disk events refresh enabled probes without resetting semantic
  indexes for metadata-only or executable-content edits. Only mutable path dependencies
  and workspace inputs acquire watchers, never hosted dependency probes. Capture/delete,
  per-input unknown scoping, warning recovery, root-config selection and watcher lifecycle
  tests cover this cutover. Options discovery skips non-files and watches no outside-workspace
  ancestors; mutable dependency watchers name exact input files and executable directories.
  Executable probes stream entries until a matching file, without a directory-size cutoff.
  Disabled rules retain main's existing dependency-source event invalidation behavior.
  Remaining exclusions: other rule-side filesystem facts remain:
  `no_deep_package_imports` reads
  dependency files; avoid_missing_test_files and conditional_uri_does_not_exist
  probe file existence; unused_assets retains the directory IO noted in V1-3c;
  path-canonicalization helpers still access the filesystem. The metadata box and
  broader frontend parity box remain open.
- [x] Close LSP manifest-diagnostic publication and declared-asset watcher parity (V1-3d).
  Initialization schedules the same cross-file pass through the existing debounce
  and outbox, publishing closed manifest findings before any Dart document opens.
  Manifest URIs retain the client's workspace namespace, including symlinked roots.
  Changed results publish once; fixes, deletion, disabling and incomplete analysis
  clear previous findings. Worker failures retain prior manifest publications until
  recovery. Ignored YAML buffers cannot replace disk-derived results or clear them
  on close; saved snapshots supply ranges and watcher/save events refresh facts.
  Asset create/delete events are relevant only for exact declared files or direct
  children of declared directories, with an enabled asset rule. Content-only changes,
  nested children and unrelated events do not schedule asset evaluation. Normalized
  declarations are cached per environment generation and share extraction with the
  rule path; deleted symlinked assets use the same path normalization.
  Asset-only events preserve semantic snapshots and workspace walks; directory
  events still refresh Dart membership. Watcher batches and configuration reloads
  coalesce cross-file evaluation through the existing debounce, holding affected
  open-document publications until merged diagnostics are ready even when later
  batches postpone the pass.
  Exact CLI/LSP parity covers sorting and secure URLs; protocol tests cover startup,
  symlinked URIs, buffer lifecycle, suppression and recovery. Watcher tests pin
  create/delete publication, unchanged semantic/walk build counts, all declaration
  forms, disabled rules and directory events. Worker-failure and unchanged-result
  tests pin retention and deduplication. Both asset CLI corpora match before and
  after sharing extraction (unused-assets: 8 expectations; missing-assets: 6).
  The broader frontend parity box remains open; dependency metadata probes remain
  excluded from V1-3e's localization capture above.
- [ ] Expand `crates/falcon_cli/tests/project_parity_tests.rs` around actual risk:
  diagnostics identity/range/severity/message, suppression, config changes,
  incomplete analysis and cross-file invalidation. Shared stages already live in
  `falcon_project`; close specific divergence rather than invent another pipeline.
- Acceptance: a project evaluation uses coherent facts, known intentional analyzer
  differences are recorded with fixtures, and no adapter duplicates rule policy.

### V1-4 — Bound editor work and preserve cancellation

- [x] Define request admission/backpressure/supersession for the unbounded worker
  channels/outbox in `crates/falcon_lsp/src/server.rs`. Cancellation already exists;
  preserve its transport path and measure latency before adding checkpoints.
- Acceptance: bounded retained queued work under a deterministic burst test;
  notifications required for document correctness are not dropped; cancellation,
  edit/version ordering, stale-result suppression and shutdown remain live.
  Choose capacities from measured workload evidence, not an invented guarantee.
- Queue inventory and policy (`server.rs` and `server/queue.rs`):
  - Relay → worker originally retained every document notification, request and
    watcher batch in an unbounded channel. The pending mailbox now folds FULL-sync
    changes by URI, keeping the last received text and version, not the maximum
    numeric version. Saves participate in text ordering without changing the last
    received version; one real `didSave` per path preserves disk refresh and path
    resolution side effects. Incremental/malformed changes remain lossless barriers;
    Falcon negotiates FULL sync, so incremental traffic is not covered by the bound.
  - Open/close lifecycle transitions are not indiscriminately discarded: within an
    epoch, open/change/close can collapse to a close transition, preserving its
    publication; a previously-open document's close/reopen keeps both transitions.
    At most three entries per document retain lifecycle, latest buffer and save
    refresh effects. Watcher batches merge across edits, unioning distinct paths;
    create/delete beats change and the latest create/delete wins ties, including
    delete/recreate. Read-only hover/code-action requests do not split folding
    epochs: they observe the latest buffer, or receive `ContentModified` when its
    version differs from the version received with the request. A newer request
    for the same method and URI cancels/supersedes the older pending request with
    `ContentModified`; no arbitrary request count or busy rejection is used.
    `codeAction/resolve`, `workspace/executeCommand` and other non-read requests
    remain lossless ordering barriers. Canceling a queued request removes it and
    recompresses its epochs. Unknown notifications remain ignored.
  - Without outstanding mutating/user actions, retained queued work is **O(D + W)**:
    D is distinct document URIs touched in the pending window (including required
    close publications), W is distinct watcher paths, and supported read methods
    per URI are a fixed protocol set (hover and code action), not a capacity.
    Outstanding user actions U preserve their own snapshots and add an honest
    **O(U · (D + W + 1))** residual across ordering epochs. Unsupported/malformed
    requests also remain barriers; unsolicited protocol floods are not covered.
    These are entry-count bounds, not bounds on text/diagnostic sizes, project
    size or final currently-open count. A 4,097-request burst interleaved with
    4,096 edits retains at most two queued reads plus one latest document entry;
    4,094 superseded reads receive `ContentModified`, and cancellation/shutdown
    still bypass the worker.
  - Worker → relay originally used an unbounded completed-round channel. A
    rendezvous now retains zero queued rounds; the worker waits for one boolean
    acknowledgement before starting the next round. A one-slot wake channel records
    only whether pending work may exist. These 0/1 capacities encode handshakes,
    not guessed workload limits. Shutdown disconnects both waits.
  - The worker-local `Connection` outbox remains an unbounded channel used only by
    one synchronous round, drained into that round before the acknowledgement wait.
    Its diagnostics, configuration warnings, responses and watcher requests cannot
    accumulate across rounds. Relay publications remain URI-keyed latest values;
    ready/other message vectors drain per round. Debounce dirty membership and
    pending semantic dependents are sets, not event histories. Configuration messages
    drain per round; reverse-dependency BFS queues visit each graph node once.
  - Dynamic watcher registration originally accumulated unacknowledged registration
    requests. Refresh now waits for acknowledgement: one initial registration or
    two replacement operations (unregister/register), then the latest desired watch
    state is recomputed. The `lsp_server` transport's own reader/writer channels are
    outside this mailbox; this does not promise bounded process memory for an
    unresponsive peer or unsolicited/malformed protocol traffic.
- Measurements on local main `b23c415d` before implementation, and this change after:
  `MBX_CACHE_LINKS=false numen run -- cargo nextest run --locked -p falcon_lsp
  -E 'test(editor_burst_measurement) | test(server::queue_tests) |
  test(request_burst_supersedes)' --no-capture`.
  The baseline numbers were collected with an **uncommitted instrumentation
  probe** of main's input channel; that baseline probe is not part of this commit.
  The committed command reproduces the after measurements. Its controlled relay
  harness holds a request pending without running a worker: it does not measure
  real analysis contention. Memory is a wire-plus-inline proxy (serialized payload
  plus inline message size), **not heap/RSS**; allocator, collection/channel
  metadata and shared keys are excluded.

  | Deterministic burst | Before: retained entries / proxy bytes | After: retained entries / proxy bytes |
  | --- | --- | --- |
  | 4,096 edits, one URI | measured 4,096 / 1,579,949 | measured 1 / 386 |
  | 4,096 edits, 64 URIs | measured 4,096 / 1,583,405 | measured 64 / 24,758 |
  | 4,096 watcher notifications, one path | measured 4,096 / 1,323,008 | measured 1 / 323 |

  Additional measured after-state bounds: 8,192 lifecycle events over 64 URIs
  retain 64 entries when initially closed, 128 when initially open; 4,100 interleaved
  open/change/save events retain two entries and publish version 2 after version 10.
  Seed-42 interleavings of 1,031 lifecycle/edit/save/watcher events over four URIs
  and four watcher paths retain 10 entries (healthy/persistent failure) or nine
  (finally closed), below the 13-entry upper bound. Seed-93 producer batches also
  interleave enqueueing with real worker pops and acknowledgements, including
  open/save → pop-open → close/reopen, and compare the same final publications
  and live episodes as unfolded processing. The original after replay recorded
  five round messages / 2,852 proxy bytes and four relay publication keys / 2,092
  proxy bytes; these are illustrative after samples, not a main outbox baseline.
  Completed-round depth zero, wake/ack slots one, per-method/per-URI read retention,
  the three-entry request/edit upper bound, and watcher pending requests at most
  two are structural bounds. The 4,096-refresh test also measures one initial
  registration and two replacement operations.
- Cancellation latency in the controlled relay-only harness, with a request held
  pending indefinitely and **no worker/analysis contention**: before 24/18/15 µs
  and after 20/18/17 µs for single/many/watcher bursts in the original recorded
  runs; the final harness rerun measured 51/23/14 µs. Other after iterations
  observed 14–525 µs; these scheduler-sensitive samples are not
  real-time guarantees or a real-worker latency bound. Cancellation stays on the
  transport path; no extra analysis checkpoints were added. Real-worker blocked-
  analysis cancellation tests retain a five-second liveness deadline, not a
  claimed production latency.
- Evidence: folded/unfolded real-worker replays compare final publications and live
  episode notifications, including transient failure (no popup), persistent failure
  (one popup), lifecycle collapse, save ordering and delete/recreate watcher merging.
  Dedicated tests cover received-version ordering, incremental/malformed barriers,
  stale results/side effects, cancellation through supersession bursts, canceled-
  request epoch reclamation and shutdown without draining a burst. Read requests
  reject advanced versions; resolving a versioned edit still computes from its
  own ordered snapshot. A worker-pop lifecycle regression checks the close clear,
  disk cache and fallback-stamp refresh before reopen. The existing no-flicker
  watcher test floods 256 watcher notifications and 256 edits in each of five
  batches: every open-file publication retains merged cross-file diagnostics and
  the final version is 1,281. Only stale-round **incompleteness** warnings stay
  Pending until a current round announces them; recovery re-arms the episode.
  One-shot configuration/environment/asset/manifest messages remain lossless,
  including an invalid-config watcher round superseded by a document open.


### V1-5 — Establish representative profiling and the performance gate

- [x] Audit `crates/falcon_rules/benches/rules_bench.rs` semantic contexts and assert
  representative semantic diagnostics before timing. Label isolated rule versus
  shared-registry costs. `benches/jfit_mobile_bench.rs` is opt-in/private and uses a
  narrower unresolved path; retain it as such, not as the release baseline.
  Contexts use production `build_semantic_indexes` and library grouping with every
  file rule enabled, but omit environment, external libraries and companion loading.
  Setup asserts exactly one `avoid-print`, `avoid-unrelated-type-casts` and
  `unnecessary-null-checks` diagnostic on `semantic.dart` in both execution lanes.
  `standalone_rule_project_facts_warm` includes one rule's own semantic/structural
  walk over 12 files, not just its visitor; these costs must not be summed as
  exclusive per-rule work. `shared_registry_project_facts_warm` includes shared
  traversal and registry postprocessing over the same files. Both time per-file
  resolution/inference. Parsing, project fact preparation, context construction and
  lazy context cache warmup are untimed; stack-pool worker handoffs are untimed.
  Package-domain rules (Flutter, Riverpod, etc.) may bail out without SDK/packages:
  the labels claim project facts, not useful resolved work for every rule.
  The private corpus path was removed; the unchanged personal bench requires
  `JFIT_PATH`. These synthetic cases make no full-analysis or release timing claim.
  Evidence uses JJ change `kqssxssk` (amending `1c18c9bf`) with benchmark-source
  SHA-256 `3606fdb8cece2072ca893062c039dc7dca9a22148e1a67f9d3b4353f5d112b34`;
  Linux x86_64, rustc `1.101.0-nightly (16d1ffac1 2026-10-26)`, cargo
  `1.101.0-nightly (cad07cf8e 2026-10-23)`, no SDK/private corpus:
  - Smoke command: `MBX_CACHE_LINKS=false numen run -- cargo bench -p falcon_rules
    --bench rules_bench -- --test`; checks both lanes without reporting timings.
  - Negative control: remove all four context `with_*` fact attachments, then run
    `MBX_CACHE_LINKS=false numen run -- cargo bench -p falcon_rules --bench
    rules_bench -- --test shared_registry`. It exits 101 before timing on the
    standalone `avoid-unrelated-type-casts` assertion (zero, expected one).
    This is a combined missing-facts control, not an independent negative proof
    for each assertion or the cross-file import; the control stops at the first
    failing rule in registry order.
    Negative benchmark-source SHA-256:
    `b92f4363dfa523afc50bf34b2041cf6265089a3fa63836d4840ac971f9c36f00`.
  - Nextest reproduction of the `makers test` all-targets path:
    `/usr/bin/time -f 'NEXTEST_BENCH_WALL_SECONDS=%e' numen run -- cargo nextest
    run --locked -p falcon_rules --all-targets -E 'binary(rules_bench)'
    --no-tests=pass`. It runs 526 test processes plus listing: before reducing
    standalone setup, 53.47 s wall / 51.443 s test time; afterward, 20.87 s /
    19.597 s, all 526 pass. Setup now runs standalone assertions only for the
    three representative rules on `semantic.dart`; shared runs warm all contexts.
    These single-host test-runner observations are not rule-performance claims.
  - Prior settled gate command: `numen run -- makers verify-fast`; its handoff
    records the result and whole-tree fingerprint before the shared-worker panic
    guard was added. The smoke command above was rerun with that guard.
- [ ] Add portable representative full-analysis cases using the existing pinned
  public corpus/bench-compare machinery, with recorded configurations, failures,
  toolchain and corpus revisions. Verify the work performed, not just duration.
- [ ] Audit stage/span attribution in `falcon_analyze/src/registry.rs`,
  `parallel.rs` and `semantic_indexes.rs`. Existing tracing is useful; distinguish
  preparation from shared traversal/index cost and keep detailed instrumentation
  opt-in. No unconditional per-node instrumentation solely to get timing numbers.
- [ ] Settle representative suite, cache conditions, repeatability/resource limits
  and acceptable regression/speed margin with the maintainer. Performance is a
  release blocker, but cold Falcon beating warm analyzer remains an aspiration
  until this decision. Keep uncached full analysis measurable; no optimization
  campaign or persistent result caching is approved merely by this checklist.
  Maintainer decision: the numerical regression margin is deferred until the
  representative suite and measurement variability are established. Performance is
  NOT deferred as a v1 release blocker. Keep uncached Falcon measurements, and define
  comparative acceptance against the Dart analyzer separately from regression
  against Falcon's own baseline.

### V1-6 — Certify the release candidate, not just rule count

- [ ] Record final `makers verify-full` evidence on the exact release candidate,
  independent review and reasons for relevant ignored/skipped tests. Run the pinned
  OSS corpus (fast and full), applicable website/typecheck/build, Dart launcher and
  editor/package/install smoke checks. The full local Rust gate alone does not
  execute these separate surfaces or establish analyzer parity.
- [ ] Review malformed-input/recovery coverage, parser depth/source limits, all
  new metadata consumers and CLI/LSP failure contracts against V1-1 through V1-4.
- [ ] Confirm documented supported Dart/Flutter/rule/config behavior, diagnostics
  and suppression semantics, migration notes and release artifact/version checks.
  Review platform CI and provenance evidence; do not publish or change credentials
  as part of cleanup. Release approval remains a separate action.
- Acceptance: every required item has source-bound evidence or an explicit
  maintainer decision reducing scope; unresolved blockers are not silently waived.

## Recommended cleanup after the enforcing slices

- [ ] R1 — Replace the `LspState → ClientCapabilities → SemanticFlags →
  WorkspaceFlags` mutable `Deref` chain in `crates/falcon_lsp/src/state.rs` with
  explicit ownership/access. Keep the primary root's registries and resolution
  requirements together instead of splitting them via `into_registries` and
  storing file/cross-file flags independently; migrate tests that set those flags
  separately so they cannot contradict registered rule requirements. This is
  maintainability debt, not by itself a proven behavioral bug. Preserve
  capability/configuration lifecycle tests.
- [ ] R2 — Remove repeated conversions/retained data only where callers and
  profiling justify them. Keep shared immutable indexes and useful caches; no
  blanket clone/Arc ban or byte-count-based redesign.
- [ ] R3 — Consider tighter lint policies incrementally where they prevent a real
  defect. Existing pedantic/strict warnings remain mandatory. Do not spend the
  readiness phase on broad style-only churn.

## Deferred unless evidence changes

Generic repository/service/DTO layers, a crate-per-layer rewrite, new task
schedulers, duplicated Numen coordination, a wholesale compiler representation
replacement, and persistent CLI analysis-result caching are not prerequisites.
Any proposal needs a concrete caller, measured problem or correctness contract,
compatibility plan and bounded acceptance criteria before implementation.
