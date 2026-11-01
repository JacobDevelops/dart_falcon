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

- [x] Agree and implement a shared completeness contract. The required-project-source
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
  External libraries with unavailable parts retain unknown, not absent, names and
  members; readable declarations remain usable. Deterministic companion tests cover
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
  anchors and episodes.
  Shared completeness contract, including the remaining external/worker classes:
  - Required source/discovery failures abort CLI analysis nonzero with path/reason,
    before a clean summary. The LSP remains live, publishes `incomplete-analysis`,
    clears obsolete cross-file findings, and suppresses the entire cross-file pass.
    Recovery re-enables rules against the current source, not retained old findings.
  - Optional metadata and external path-dependency/SDK/pub-cache reads warn without
    failing the project. Missing/non-file directive targets remain silently absent,
    but unavailable external parts make dependent name/member absence unknown.
    Manifest snapshots, package configuration, analysis-options includes, localization
    inputs, native-plugin/resource/build-tool facts and barrel-export inputs retain
    unknown outcomes: their dependent checks skip or conservatively exempt, never
    infer absence from an acquisition/parse failure. Package configuration distinguishes
    missing, unknown and valid inputs: invalid/partially invalid entries cannot establish
    conditional-URI or dependency absence. Valid entries in a partial configuration
    retain positive package roots, language versions and resolved import/test facts.
    Independent rules remain eligible; optional failures do not publish
    `incomplete-analysis` or turn the CLI exit nonzero.
  - Caught parser/rule/semantic-index/cross-file worker failures are never optional:
    CLI exits nonzero even when the configured diagnostic exit code is zero.
    Failure diagnostics name the source and worker failure; the LSP adds
    `incomplete-analysis`, suppresses cross-file rules, and replaces stale manifest
    publications too. Successful retries rebuild failed semantic/external facts and
    publish current diagnostics even when only one of several documents changes.
    External worker failures follow current import/part reachability: removing the
    failing dependency clears qualification without a dependency-file edit; restoring
    its directive retries it. Separate failures retain their own source fingerprints.
    Current worker state is separate from previously published diagnostics. Parser
    injection is unit-test-only, or gated by `test-support` and debug assertions for
    frontend subprocess regressions; neither seam is present in release builds.
  - Fatal controller-thread panics and native stack-overflow aborts terminate the
    process nonzero, rather than leaving a live server with stale publications.
    Cargo profiles retain unwinding; the relay propagates the controller join error
    without waiting for client shutdown. `controller_panic_exits_nonzero_without_waiting_for_client_shutdown`
    spawns the real binary with a test-only/debug-only seam and holds stdin open.
    The parser's 150-frame guard and `recursive_robustness_tests`/deep interpolation
    regressions prevent adversarial nesting from exhausting the native stack.
    Recovery from fatal termination is a fresh process/client restart.
  - No located SDK is supported syntax/model-facts mode, not a required-source
    failure; unresolved SDK facts remain unknown. Future package language versions
    warn once through CLI/LSP configuration warnings and use the latest supported
    version, without marking analysis incomplete.
  Evidence includes CLI/LSP optional-external-part tests for disappearing/missing,
  invalid-UTF-8, oversized and deterministic unreadable symlink-loop sources: an
  independent `unused-files` finding remains eligible while an unknown doc reference
  is exempt, then recovery produces a new `comment-references` finding. SDK external
  part tests assert `DocName::Unknown`; compact external tests assert unknown members.
  External parser panic/disappearance tests require fresh recovered declarations.
  CLI completion tests and LSP file-rule, cross-file-rule, project-parser and semantic
  panic tests independently assert nonzero/qualification, cross-file suppression and
  current-source recovery, including `change → analyze_affected_many → cross_file_pass`
  with two documents. External parser subprocess/autofix and both LSP semantic-view
  tests assert file-specific failure, suppression and current-source recovery.
  Broken package-config tests assert warning/exit/publication behavior, unknown
  conditional URIs, and independently expected normal/dev unused-dependency findings
  after repair; future-version tests assert warning-once behavior and no incomplete signal.
  Partial-entry tests independently assert Dart 2.19 break behavior, positive package
  roots/language versions and safe LSP index reuse; focused rule tests distinguish
  unavailable configuration from complete absence. Warm-cache semantic failures
  prove external-layer reset, and separate autofix tests exercise companion reparse
  failure and the nonsemantic CLI fix-companion path before any writes.
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

- [x] Move rule-side metadata acquisition behind a shared bounded project snapshot.
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
  V1-3c initially retained `unused-assets`' directory expansion and regular-file
  checks outside the manifest snapshot; V1-3g moves these raw directory facts into
  the shared generation/event owner below. `missing-assets` still checks declarations,
  not disk existence.
  Missing/non-directory paths are silent and exempt; dangling symlinks skip only
  that entry. Other directory/entry IO errors and non-UTF-8 file names exempt the
  directory and warn with path and reason through the environment warning channel,
  deduplicated by declared directory until a successful refreshed directory read or
  discovery reset. A successful refresh re-arms that directory's warning, so a
  subsequent failure warns again without an environment reset. CLI subprocess tests cover the
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
  V1-3g closes the remaining rule-side filesystem exclusions. The environment lazily
  memoizes bounded dependency export syntax, regular-file existence, immediate
  directory file names (at most 100,000 entries), and canonical package paths.
  Rules retain export, URI, test-file and asset policy. Missing inputs, including
  non-directory ancestors and dangling entries, remain silent. Malformed/inaccessible
  inputs warn once per failure episode and make only their dependent consumer unknown;
  an unknown dependency export invalidates that barrel's deep-import proof rather
  than preserving a partial closure. Dependency source acquisition is bounded to
  8 MiB, with the existing 512-library/8 MiB traversal budget retained in the rule.
  Oversized and Dart-parse-error sources retain main's silent per-file skip, so an
  unrelated oversized/broken sibling cannot hide a small valid export. Source facts
  retain only byte lengths and raw export URI/flag records, not parsed ASTs or source
  buffers; unchanged idle polls compare size/mtime, permissions and Unix ctime without
  parsing dependency sources. The remaining closure budget is checked before reads.
  Existence/canonical memos retain at most 256 entries. Enabled existence consumers
  watch bounded workspace roots rather than accumulating one watcher per typed URI;
  events match captured keys and their canonical/alias ancestors. Eviction retains
  at most 256 parent directories for event matching; workspace-root fallback applies
  only if those directories overflow too. Idle polls remain quiet on unchanged disk
  state. Failure episodes outlive memo eviction; unrelated fact kinds cannot re-arm them.
  Raw directory facts retain the per-directory entry bound.
  Standalone rule contexts share one fallback facts owner rather than creating one
  environment per rule preparation; deprecated-member and path-pattern behavior is
  unchanged when no project environment is supplied.
  Canonical comparisons retain separate package-path and lexical-fallback semantics;
  the latter reuse captured source paths before acquiring other path facts.
  CLI runs capture fresh facts; normalized LSP save/watcher paths refresh affected
  file/directory memos. Existence changes reanalyze open documents; Dart and directory
  events also refresh project membership. Mixed event batches retain the deferred
  cross-file publication pass. Explicit workspace boundaries include pubspec-less
  documents; alias/canonical and symlinked-directory events invalidate the same facts.
  Newly captured facts acquire watchers only inside analyzed project boundaries,
  never outside-workspace/hosted dependency paths.
  Deleted-after-capture, unchanged memo identity, outside-boundary watcher exclusion,
  create/delete publication for missing-test/conditional-URI diagnostics, conservative
  unreadable dependency sources, dangling assets and warning recovery have regressions.
  Before/after full diagnostic JSON matched for 112 corpus invocations across all 18 directly changed rules and shared barrel consumers.
  Filesystem audit command:
  `rg -n 'std::fs|read_dir|metadata\(|\.is_file\(|\.is_dir\(|\.exists\(|canonicalize|File::open' crates/falcon_rules/src`.
  Result: **zero production project-fact acquisitions**. Remaining hits are test-only
  fixture IO; cached `FilePackage::metadata()` getters; the conditional URI resolver's
  shared-fact query; warning emission; configuration/option/rule-name normalization;
  compile-time metadata combination; or documentation/examples. Path normalization
  in rule code is lexical or delegates to the environment's canonical facts. The
  broader frontend parity box remains separate and open.

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
- [x] Expand `crates/falcon_cli/tests/project_parity_tests.rs` around actual risk (V1-3h).
  Real in-memory LSP transport publications match CLI collection plus JSON formatting
  for rule IDs, exact messages, all three severities and byte-to-UTF-16 ranges,
  including accented and astral characters before a diagnostic. Table cases pin
  inline suppression and cross-file file suppression; existing full-project cases
  retain path overrides, exclusions and merged file/project findings.
  Dynamically registered watchers reload explicit and workspace-discovered Falcon
  configuration through warning → error → disabled → info, matching a fresh CLI
  evaluation with the fixture config explicitly selected at every step.
  Ordering barriers drain publications and reject missing/duplicate target results.
  Saved open-file import edits use a long debounce to distinguish save from change;
  valid closed importer creation/deletion changes another document's reachability.
  Invalid UTF-8 in a newly discovered required source aborts the CLI nonzero and
  replaces stale cross-file warnings with the exact incomplete-analysis signal;
  deletion restores fresh CLI parity. Existing companion/traversal and manifest
  fixtures retain acquisition-specific recovery coverage.
  Comparison selects each file and drops non-identity metadata (fixes, LSP source
  and document version), normalizing URI/path, severity encoding and byte/UTF-16
  positions. It does not exercise the CLI package-aware output renderer.
  Valid source deletion exposed a watcher fast-path divergence: cached project
  paths were treated as dependency metadata when no semantic snapshot existed.
  The LSP now retains those paths in project invalidation, restoring reachability
  findings instead of a stale missing-source incomplete-analysis signal.
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
- [x] Add portable representative full-analysis cases using the existing pinned
  public corpus/bench-compare machinery, with recorded configurations, failures,
  toolchain and corpus revisions. Verify the work performed, not just duration.
  - Reproduce: `cargo xtask bench-compare --full-analysis --runs 3 --output
    target/full-analysis.json`. The [portable lane contract](verification.md#portable-full-analysis-measurements)
    describes the checks and limitations; `xtask/full-analysis-cases.json` selects
    existing e2e pins. `xtask/full-analysis-expected.json` asserts host-independent
    work counters and per-rule findings; `--bless` updates reviewed expectations.
  - LocalSend app at `cfd699125f2aa4c69f48fd274cf4bb1876a5e08d`:
    294 files / 146,823 diagnostics; bloc pure-Dart `packages/bloc` and
    multi-package `packages` at `b9be1e252ee24ffbe9f2fb629edc7624b2cfbf05`:
    43 / 2,323 and 209 / 15,207 respectively. Every case verified 525 file rules,
    21 completed cross-file rules, resolution, the expected single semantic index
    build, and completed analysis/preparation for every expected file before and
    after timing. Explicit all-rule warning configuration, four threads, SDK absent,
    no pub get, zero external libraries and a clean checkout are enforced.
  - 2026-11-02, Linux 7.0.0-38-generic x86_64, Ryzen 7 9800X3D,
    rustc `1.101.0-nightly (16d1ffac1 2026-10-26)`, cargo
    `1.101.0-nightly (cad07cf8e 2026-10-23)`: one warmup, three fresh-process
    uninstrumented samples; median/range ms were LocalSend 1,991.655 /
    1,989.941–2,022.495, bloc 100.899 / 98.339–106.448, workspace
    422.088 / 418.342–422.126. Instrumented preflight peak RSS (decimal MB):
    843.969 / 41.394 / 85.254. Uncached analysis, retained OS page cache; failures
    empty. Evidence belongs to jj change `muponswvrzux` (measured source commit
    `603c2f69ef90426c65ca55e76799d6512840fa87`);
    binary SHA-256 `349b263582c085fbbfb0a64ca352bda02a8db3071bbe35f12195ae931e54a9aa`.
    Complete JSON rendering is deliberately included in end-to-end timings:
    a review probe capped at one emitted finding reduced LocalSend wall time
    about 15%, so these are not analysis-only numbers. Machine-readable host
    timings are output artifacts, not committed baselines or an accepted margin.
- [x] Audit stage/span attribution in `falcon_analyze/src/registry.rs`,
  `parallel.rs` and `semantic_indexes.rs`. Existing tracing is useful; distinguish
  preparation from shared traversal/index cost and keep detailed instrumentation
  opt-in. No unconditional per-node instrumentation solely to get timing numbers.
  - The [stage attribution inventory and capture recipe](verification.md#attributing-analysis-time)
    covers CLI, project, registry, parallel workers, shared indexes and the LSP
    cross-file pass. Preparation, shared walks, semantic state/index construction
    and reporting have separate boundaries. First-use per-file index spans
    identify shared facts initialized inside preparation, semantic state or
    callbacks; body-only semantic refreshes are attributed separately.
    File/rule detail is
    DEBUG-only on `falcon::detail`; DEBUG-level `falcon::stages` is opt-in, even
    under ordinary INFO logging. No per-node spans or timers were added.
    Inactive profiling skips dispatcher installation; scoped-subscriber
    embedders must observe the caveat in the capture recipe.
  - Test subscribers assert exact per-file stage counts over 64-file sequential,
    Rayon resolving/non-resolving and large-stack execution, including work
    stealing, the real LSP cross-file index/rule transfers and project autofix
    reanalysis. They assert
    preparation/traversal siblings, first-use cache spans (no repeat on cache
    hits), body refreshes, subscriber-only capture without an entered parent,
    and the real CLI default filters. Ordinary INFO emits no stages/detail.
    No durations are asserted. Removing all three Rayon context restorations
    loses parse spans; removing just indexed rule-worker restoration loses
    preparation spans, and the strengthened tests reject both mutations.
    Reverting the LSP cross-file index transfer to a bare parser stack also
    fails the LSP test.
    Analyze stage-capture tests, project autofix capture and LSP cross-file capture
    re-execute themselves in isolated child processes, preserving production's
    inactive fast path while excluding unrelated libtest jobs from the subscriber.
    Each child must succeed and report exactly one test; no stage-count assertions
    were relaxed.
    The full `falcon_analyze --test it` suite passed 20 consecutive default-thread
    libtest runs, with 110 tests per run.
    `falcon_project --lib` (62 tests) and `falcon_lsp --test it incremental_tests`
    (21 tests) each passed 10 consecutive default-thread libtest runs.
    CLI JSON stage-close smoke emitted 4,419 records across 15 stage names,
    including lazy indexes, with busy/idle values and no detail.
  - Default-path comparison used release builds of `bebbf66c` and this slice,
    with the same locked dependencies, in
    separate scratch target directories outside the source tree. Corpus:
    LocalSend `cfd699125f2aa4c69f48fd274cf4bb1876a5e08d`, `app/`;
    `falcon check --quiet <app>`, `RUST_LOG` unset, `RAYON_NUM_THREADS=4`,
    stdout/stderr discarded. This performed 294 files, 385 rules, 17 cross-file
    rules and resolution enabled, with 11 external libraries. Separate JSON
    runs produced identical findings (canonical JSON SHA-256
    `9c9aa17d3d150476e430a4b4c00829e3ca4be58968c3fcafe24e6d6cd22df941`;
    36,788,212 bytes after sorting JSON keys with compact separators; paths
    retain the shared scratch corpus prefix).
  - Fifteen alternating before/after pairs on the final revised binaries,
    after one warmup per executable, gave wall medians
    **1,282.649 ms before / 1,298.601 ms after (+1.24%)**; child CPU medians
    **2,690.739 / 2,707.843 ms (+0.64%)**. Wall ranges were
    1,274.834–1,337.184 / 1,294.281–1,315.131 ms. Other heavy workspace work
    was paused for this batch. Earlier conditional-capture attribution batches
    gave +0.52%, +1.44% and -0.18% wall over fifteen pairs, and +4.11% over
    seven pairs. An unconditional dispatcher-restoration candidate measured
    +4.08% wall / +4.23% CPU and was rejected. These observations are not a new
    numerical regression allowance or universal overhead bound.
    Every sample was a fresh uncached Falcon process, with warm OS page cache,
    not a cold filesystem or persistent analysis cache. Host: Linux
    7.0.0-38-generic, Ryzen 7 9800X3D, rustc 1.101.0-nightly
    (`16d1ffac1`, LLVM 23.1.4); no affinity pinning. These are attribution-audit
    observations, not analyzer parity or release performance acceptance.
    The remaining V1-5 decisions below are unchanged.
- Dependency-resolved benchmark evidence (2026-11-02; **not performance acceptance**):
  - The default `--full-analysis` lane still selects the three portable cases.
    `--setup-resolved` prepares pinned Bloc and Wonderous workloads; `--resolved`
    selects them explicitly. See [setup/cache contracts](verification.md#resolved-workloads-and-analyzer-comparison).
    Mise pins Dart 3.13.6 and Flutter 3.47.7-stable (Dart 3.13.5); SDK acquisition,
    corpus checkout and enforced dependency resolution are outside timed runs.
    Bloc resolves 56 package-config entries and 268 external libraries over 43
    inputs; Wonderous resolves 134 entries and 2,291 external libraries over 192
    inputs after fresh setup. Exact per-rule findings and completed-work counts are
    blessed in `xtask/full-analysis-expected.json` (2,995 and 26,320 findings).
    Missing/changed setup, unexpected ignored content, missing package roots,
    zero/changed external-library work, missing/nonzero structured external
    parse-error counts, warning/error logs and `incomplete-analysis` diagnostics
    fail the lane. Setup uses Flutter's bundled Dart with `FLUTTER_ROOT`, avoiding
    Flutter CLI migrations/generated inputs; `.dart_tool/**` is excluded.
  - Host: Ryzen 7 9800X3D, 8 cores/16 threads, 49.569 GB RAM, Linux
    7.0.0-38-generic x86_64; rustc 1.101.0-nightly (`16d1ffac1`, LLVM 23.1.4),
    Hyperfine 2.0.0, release Falcon SHA-256
    `0088ae7b38228916d8a9975b89f99fd44935e66066aab67fe02d5ebb5d0f7bb7`.
    Falcon used four Rayon threads; no CPU affinity or governor lock.
    All five Falcon rows were timed with the pre-R2 binary (`0088ae7b` SHA-256
    prefix), before the R2 `max-lines-for-function` optimization (`ecaee7c3`).
    The controller confirmed that the rebased binary (`0c08a00a` SHA-256 prefix)
    still passes the resolved lane, so the blessed work holds; the table is not
    a timing measurement of that rebased binary.
    LocalSendProfile confirmed its verification gate had finished before timing
    and held heavy work until `resume heavy`. All four corrected N=10 suites were
    rerun after fresh resolved setup and warm-cache convergence enforcement.
    Earlier contaminated/generated-input and unstable-warm batches are superseded.
    No other heavy work was reported; unrelated desktop load was not isolated.
    Process swap is rejected by the measurement guard.
  - Commands: after explicit setup, run
    `cargo xtask bench-compare --full-analysis --skip-build --runs 10 --output <report>`
    for portable Falcon, add `--resolved` to check the separately blessed expected
    work for the resolved cases (no blessing during this replacement batch);
    separately use `--full-analysis --analyzer-comparison --runs 10 --output <report>`
    with and without `--resolved`. Execution used the copied xtask binary through
    `numen run --`; all four commands passed. Every
    row below has **N=10**. Falcon has one discarded warmup, then fresh cacheless
    processes with warmed OS page cache; Falcon warm is **not applicable**.
    Analyzer cold clears private persistent state before every sample and discards
    none; warm retains populated `.analysis-driver` state only after untimed
    population converges to a stable nonempty file count, also in fresh processes.
    Both tools render full JSON; their rule sets and file-selection semantics
    are not equivalent. Native analyzer findings were stable across each case:
    12,155 / 1,828 / 10,160 for the unresolved portable cases, zero for resolved
    Bloc, and 105 ordinary warnings/info findings for Wonderous (no missing-import
    or unresolved-package diagnostics).

    | Case | Tool/cache | Median ms | MAD ms | IQR ms | CV |
    | --- | --- | ---: | ---: | ---: | ---: |
    | localsend-app | Falcon cold | 1956.575 | 2.581 | 10.344 | 0.57% |
    | bloc-dart | Falcon cold | 103.718 | 1.572 | 3.427 | 2.71% |
    | bloc-workspace | Falcon cold | 388.751 | 5.257 | 9.499 | 1.58% |
    | bloc-dart-resolved | Falcon cold | 183.291 | 3.308 | 5.511 | 2.32% |
    | wonderous-flutter-resolved | Falcon cold | 1632.351 | 15.666 | 26.664 | 1.00% |
    | localsend-app | Analyzer cold | 926.040 | 6.916 | 21.919 | 1.51% |
    | localsend-app | Analyzer warm | 265.025 | 3.918 | 6.890 | 2.05% |
    | bloc-dart | Analyzer cold | 319.091 | 6.399 | 12.122 | 3.82% |
    | bloc-dart | Analyzer warm | 76.553 | 1.508 | 2.910 | 6.47% |
    | bloc-workspace | Analyzer cold | 885.258 | 5.269 | 8.652 | 1.41% |
    | bloc-workspace | Analyzer warm | 277.255 | 2.976 | 4.918 | 1.33% |
    | bloc-dart-resolved | Analyzer cold | 480.821 | 4.229 | 7.221 | 2.57% |
    | bloc-dart-resolved | Analyzer warm | 83.481 | 2.680 | 4.687 | 3.63% |
    | wonderous-flutter-resolved | Analyzer cold | 6882.933 | 82.804 | 126.681 | 1.51% |
    | wonderous-flutter-resolved | Analyzer warm | 832.363 | 3.842 | 8.809 | 0.68% |

  - Statistics use the ordinary median, unscaled median absolute deviation,
    linearly interpolated quartiles and sample-standard-deviation/mean CV.
    Warm population required two untimed invocations for LocalSend, portable Bloc,
    Bloc workspace and resolved Bloc; their counts settled at 1,460 / 343 / 1,352 /
    984 files. Wonderous required five invocations: 4,099 → 5,332 → 5,997 → 6,037 →
    6,037. All ten measured warm samples for every case retained the settled count
    before and after execution. The harness fails if population does not settle
    by `max(1, --warmups) + 10` invocations or any measured warm count drifts.
    Count stability is not proof of unchanged bytes or complete persistence.
    Cold cache writes still varied (resolved Bloc 310–702 files, Wonderous
    1,604–2,203); cold samples intentionally start empty and are not discarded.
  - **Candidate self-regression tolerances, not selected policy:** per-case
    `3 × MAD / median` gives LocalSend 0.40% (7.742 ms), portable Bloc 4.55%
    (4.715 ms), Bloc workspace 4.06% (15.771 ms), resolved Bloc 5.41% (9.925 ms),
    resolved Wonderous 2.88% (46.997 ms). The alternative `3 × IQR / median`
    gives 1.59%, 9.91%, 7.33%, 9.02%, 4.90%, respectively. These are
    observed single-host noise scales, not confidence bounds or approved margins;
    an assumed universal 5% allowance is unsupported by this batch. Self-regression
    and analyzer comparison remain separate acceptance criteria. No comparative
    speed target is chosen here; cold Falcon beating warm Dart is still the stretch
    goal, not an informational replacement for an agreed release target.

    Raw wall samples in seconds, in observation order:

    ```text
    Falcon cold localsend-app: 1.955049339 1.967905949 1.953130803 1.957282864 1.954858057 1.948181573 1.983915367 1.956297977 1.956852089 1.974973640
    Falcon cold bloc-dart: 0.107671578 0.106210559 0.105128809 0.104508603 0.109318891 0.102379577 0.100060612 0.101985331 0.102912377 0.102926534
    Falcon cold bloc-workspace: 0.390919507 0.386454753 0.382652216 0.382389929 0.387836774 0.396554810 0.384335948 0.389665211 0.400233204 0.395513337
    Falcon cold bloc-dart-resolved: 0.181844702 0.182132949 0.188768447 0.175216338 0.184201089 0.188138262 0.179974107 0.182379997 0.187706193 0.186590540
    Falcon cold wonderous-flutter-resolved: 1.628818292 1.620850525 1.658497486 1.634000577 1.613910430 1.645241808 1.630701973 1.612273175 1.650927376 1.652174899
    Analyzer cold localsend-app: 0.908077301 0.897092479 0.900636723 0.910694120 0.928154489 0.923925736 0.931915640 0.933997394 0.931403989 0.928389875
    Analyzer warm localsend-app: 0.273473923 0.274268424 0.264949689 0.261237344 0.257312084 0.260893139 0.263207067 0.269073032 0.265099787 0.267259563
    Analyzer cold bloc-dart: 0.323585853 0.314596959 0.314580118 0.306634580 0.312145940 0.301519626 0.324672161 0.327850333 0.344186227 0.324944179
    Analyzer warm bloc-dart: 0.077632122 0.072797783 0.077928796 0.090501838 0.075938099 0.077883011 0.077167541 0.074913774 0.073256467 0.074908465
    Analyzer cold bloc-workspace: 0.874854961 0.868417774 0.891916970 0.884502279 0.900578402 0.887412229 0.884419233 0.881378228 0.886013330 0.912560806
    Analyzer warm bloc-workspace: 0.273975133 0.273581403 0.277043467 0.279503777 0.281175136 0.274581413 0.283004466 0.277465696 0.271026070 0.277666925
    Analyzer cold bloc-dart-resolved: 0.501496085 0.475976083 0.476744516 0.482082944 0.481868685 0.478575458 0.479772494 0.512078172 0.485202756 0.472308866
    Analyzer warm bloc-dart-resolved: 0.089359074 0.080019468 0.080614755 0.081634652 0.083567620 0.083394415 0.086769539 0.086682275 0.085974865 0.082369781
    Analyzer cold wonderous-flutter-resolved: 6.801503824 6.718097598 6.765001900 6.881620140 6.876062620 7.031245795 6.967111042 6.885964977 7.027680144 6.884246163
    Analyzer warm wonderous-flutter-resolved: 0.825985668 0.831228044 0.829487180 0.839078881 0.840990545 0.828929911 0.834277374 0.833497500 0.828112610 0.842086584
    ```

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
  Resolved Dart/Flutter workloads and the above repeatability evidence now exist;
  the representative-suite decision, regression margin and explicit comparative
  release target still require maintainer agreement.

### V1-6 — Certify the release candidate, not just rule count

- [ ] Record final `makers verify-full` evidence on the exact release candidate,
  independent review and reasons for relevant ignored/skipped tests. Run the pinned
  OSS corpus (fast and full), applicable website/typecheck/build, Dart launcher and
  editor/package/install smoke checks. The full local Rust gate alone does not
  execute these separate surfaces or establish analyzer parity.
  - Rehearsal (2026-11-01, Linux x86_64, falcon 0.4.6; **not RC certification**):
    OSS, website, Dart, Zed and native install checks ran on main `7c4d4480927a`;
    VSIX checks ran on main plus this candidate's packaging/README fixes.
    Standalone Rust checks and the fast gate ran on main plus the candidate.
    Heavy commands below ran through `numen run --`; Cargo used
    `MBX_CACHE_LINKS=false`. No snapshots were changed.
    - `cargo xtask e2e --fast` and `cargo xtask e2e` passed all pinned
      per-rule snapshots. Diagnostic totals: immich 334, appflowy 3,358,
      flutter-packages 42,593, flame 405, localsend 11,730, bloc 459,
      riverpod 6,128, wonderous 514, flutter-samples 17,465,
      dart-sdk-language 170,207, very-good-analysis 3. Fast covered
      localsend/bloc/wonderous. No internal errors; the SDK's 16,995 intentional
      negative-fixture syntax errors matched the baseline, with zero elsewhere.
    - Website: `bun install --frozen-lockfile`, `bun run typecheck` passed;
      its `pretypecheck` ran the production client/SSR build and generated docs
      synchronization. Bun 1.4.4; no source or generated-content changes needed.
    - Dart launcher (`packages/falcon`): `dart pub get`,
      `dart analyze --fatal-infos`, `dart test` passed (15 tests, Dart 3.13.6).
    - VS Code: `npm ci`, `npx --yes @vscode/vsce package --out <scratch>.vsix`
      passed after adding missing repository metadata (relative README links
      previously aborted packaging), replacing relative links with absolute URLs
      (vsce ignores the monorepo repository directory), and documenting a fixed
      `falcon-vscode.vsix` filename for packaging/installing.
      Packaged 0.4.6 VSIX passed CRC, entrypoint/dependency presence and Node syntax
      checks; all three links in `extension/readme.md` were inspected and fetched.
      Packaging warns about missing LICENSE and bundling opportunities;
      these are not fatal. No extension build/test script exists.
    - Zed: `cargo build --locked --manifest-path extensions/falcon-zed/Cargo.toml
      --target wasm32-wasip2 --release` passed; copied artifact's WASM header
      validated (222,364 bytes). No separate package/test script exists.
    - Install smoke: the release binary built by the OSS runner passed
      `--version` and `check crates/falcon_rules/tests/corpus/avoid-dynamic/good.dart`
      (warnings expected; exit 0). SHA256
      `3535e1a6b6ff1caf066c57969a39a1ce9352a17cb2d9da6bd716dc372d9e007f`.
      The documented tarball install has no offline installer; no releases were
      downloaded/published and no global installation was changed.
    - Skip inventory: the only `#[ignore]` tests are
      `falcon_analyze::semantic_indexes::declaration_index_update_baseline`
      (release timing benchmark) and
      `falcon_analyze::resolve::semantic::fixpoint_bench::inference_index_baseline`
      (manual release inference baseline). Both are justified performance probes,
      not omitted correctness coverage. `.config/nextest.toml` has no skip filters.
      Permission-bypass guards conditionally omit assertions in
      `dependency_permission_failures_are_unknown_when_enforced`,
      `unreadable_asset_directory_warns_once_per_episode_and_is_exempt`,
      `unreadable_directory_blocks_cli_and_editor_results_and_recovers`,
      `unreadable_directories_fail_only_when_included_descendants_are_possible`
      and `unreadable_directories_fail_unless_excluded`: a user bypassing Unix
      mode bits cannot exercise EACCES. Windows-only
      `windows_prefix_spellings_preserve_internal_links_without_filesystem_probes`
      and `canonical_windows_root_matches_non_verbatim_document_parents`
      cannot execute on this host. Unix/64-bit gates are applicable here;
      `controller_panic_exits_nonzero_without_waiting_for_client_shutdown`
      requires debug assertions (the fast gate's test profile enables them).
      Subprocess-helper early returns and Dart corpus `skip: true` examples are
      harness/fixture mechanics, not skipped assertions. No unjustified skip found.
      `no_new_hand_rolled_syntax_walkers` replaces assertions only when explicitly
      updating its baseline with `UPDATE_BESPOKE_WALKER_BASELINE` (not set here).
      `test_find_config_none_when_no_local_or_git_config` omits its no-config
      assertion when `$HOME/.falcon.json` exists because home fallback is valid.
      Release-only behavior in `context_rejects_invalid_rule_fixes` (discarding
      invalid fixes instead of debug panic) and the 100ms ceiling in
      `incremental_reanalyze_meets_latency_gate` (400ms in debug) was not exercised
      by the debug fast gate; the actual RC must cover these release branches.
    - Standalone workspace Clippy (`--locked --workspace --all-targets -- -D warnings`),
      `cargo fmt --all --check` and `cargo xtask docgen --check` passed.
      The initial `makers verify-fast` passed: 3,673 tests, two ignored timing
      probes, 18 doctests; intermediate-tree source fingerprint
      `sha256:bb266d82c16e23a5e0a3534b14ee176778a94c65b79740dd5e37ab4ef411c3e7`.
      That run preceded independent-review README/evidence corrections. One
      controller-authorized replacement fast gate covers the corrected candidate;
      its fingerprint is recorded in the handoff rather than inserted into its
      own fingerprinted input.
    - Actual RC still needs its exact final `makers verify-full` and independent
      review, native macOS/arm64 and Linux arm64 install checks, published artifact
      provenance/signature/launcher checks, and interactive VS Code/Zed diagnostic
      and configuration smoke. VS Code is absent here; Zed compilation/artifact
      checks do not establish editor-host behavior. This rehearsal does not claim
      analyzer parity or replace those checks.
- [x] Review malformed-input/recovery coverage, parser depth/source limits, all
  new metadata consumers and CLI/LSP failure contracts against V1-1 through V1-4.
  Certification: JJ change `voysnztnpzoostkrtvztyvzttlztnlxl` (failure-contract fix).
  Evidence matrix (CLI/editor share project and metadata owners unless a frontend
  test is named; transport-only rows are intentionally not CLI contracts):
  - Malformed/unterminated Dart, expressions/blocks and numeric escapes →
    `malformed_dart_frontends_recover_to_current_diagnostics`,
    `invalid_numeric_escapes_preserve_following_declarations`,
    `escaped_line_breaks_reject_single_line_strings_but_preserve_triple_strings`,
    `nested_expression_forms_skip_past_the_depth_limit`,
    `deeply_nested_interpolations_hit_depth_limit_without_overflow`; malformed
    numeric escapes now report syntax errors without swallowing later declarations.
  - Source admission/UTF-8 → `oversized_sources_abort_before_partial_workspace_analysis`,
    `rejected_buffer_diagnostic_survives_refresh_and_recovers_on_change`,
    `oversized_required_sources_clear_cross_file_results_but_optional_manifests_do_not`,
    `required_source_failure_blocks_partial_projects_and_recovers`. Decision: use
    sparse oversized disk files and injected lengths through real `open`/`change`
    admission, not allocate a multi-GiB JSON message for transport serialization.
  - BOM/CRLF/UTF-16 → `dart_bom_crlf_and_astral_ranges_have_independent_protocol_expectations`,
    `localization_bom_preserves_cli_byte_spans_and_report_fingerprints`; explicit
    editor BOM is a UTF-16 code unit, unlike the disk encoding marker.
  - Pubspec/package config → `optional_manifest_failures_recover_to_fresh_secure_url_diagnostics`,
    `dependency_manifest_consumers_recover_exact_diagnostics_and_warning_episodes`,
    `environment_reports_malformed_and_unreadable_package_inputs`,
    `package_entry_failures_keep_valid_packages_and_report_details`. Decision:
    package-entry partial success is tested in the shared environment owner, not
    duplicated as frontend wiring assertions.
  - Analysis options/Falcon config → `malformed_optional_settings_recover_current_frontend_findings`,
    `dependency_documents_reject_invalid_utf8_and_oversize`,
    `invalid_falcon_configuration_aborts_cli_but_editor_retains_and_recovers_rules`;
    broken optional options skip their consumer; invalid explicit CLI config aborts,
    while editors retain valid rules and recover on reload.
  - l10n.yaml/ARB/assets → `malformed_and_inaccessible_localization_inputs_warn_once_and_skip`,
    `oversized_localization_inputs_are_bounded_and_recovery_rearms_warning`,
    `unused_localization_cli_lsp_parity_tracks_disk_facts_and_warning_recovery`,
    `duplicate_key_manifests_warn_and_skip_both_asset_rules`,
    `asset_directory_failures_reach_config_messages_once_per_episode`.
  - Dependency facts/exports/external sources →
    `dependency_probe_unknown_is_scoped_to_its_input_and_consumer`,
    `dependency_manifest_warning_rearms_after_recovery`,
    `unreadable_dependency_source_is_unknown_and_warning_rearms_after_recovery`,
    `oversized_and_parse_error_siblings_do_not_hide_small_exports`,
    `external_source_read_and_parse_failures_warn_once`. Decision: optional
    unavailable external sources remain conservative unknowns, not required-source
    fatal errors; valid siblings and warning episodes are independently asserted.
  - Required unreadable/disappearing sources/discovery →
    `unreadable_project_companions_abort_checks_and_writes`,
    `failed_required_disk_reads_are_incomplete_but_optional_reads_are_not`,
    `unreadable_directory_blocks_cli_and_editor_results_and_recovers`,
    `failed_discovery_is_incomplete_until_roots_recover_or_disappear`,
    `disappearing_traversal_inputs_are_absent`; genuinely absent traversal entries
    are not failures, while required reads block incomplete cross-file results.
  - Workers → `worker_failure_forces_nonzero_and_skips_cross_file_until_recovery`,
    `project_parse_failures_publish_once_skip_cross_rules_and_recover_on_disk_events`,
    `cross_rule_worker_failure_qualifies_manifest_publication_until_recovery`.
  - Editor backpressure/cancellation →
    `request_burst_supersedes_matching_reads_without_blocking_cancellation_or_shutdown`,
    `cancellation_responds_while_analysis_is_blocked_and_suppresses_late_success`,
    `lifecycle_bursts_retain_at_most_two_transitions_per_document`.
  Throwaway smoke: 1,028 truncated prefixes from 94 corpus files, no parser
  panic/timeout; the script and inputs were not retained as a reproducible suite.
  Reviewed nightly corpus change: `dart-sdk-language` syntax errors 16,987 →
  16,995, exactly eight analyzer-annotated invalid Unicode cases in
  `tests/language/string/unicode1_test.dart` through `unicode4_test.dart` at pinned
  SDK revision `203af6d3`. Updated only this reviewed snapshot count rather than
  rerun the entire multi-project e2e corpus; independent review compared base and
  candidate on this corpus and found no other rule-count changes.
  Diagnostic precision decision: malformed escapes retain the existing whole-token
  parser error; escape-local messages/ranges require a separate lexer diagnostic
  channel and are not part of this recovery/admission certification.
- [ ] Confirm documented supported Dart/Flutter/rule/config behavior, diagnostics
  and suppression semantics, migration notes and release artifact/version checks.
  Review platform CI and provenance evidence; do not publish or change credentials
  as part of cleanup. Release approval remains a separate action.
  Contract audit at `334dcfbe0066c667f4eb4d85cba0c9e1dabcaa0c`: inventoried
  `docs/*.md`, README/changelog/changeset policy, generated website data,
  launcher/editor manifests and all six release/CI workflows. Schema and docgen
  checks agree with 546 rules, 215 recommended rules and 21 cross-file rules.
  Config/CLI/LSP/syntax/analyze/xtask suites passed (1,534 tests; two skipped),
  including version-pin, migration, SDK/Flutter resolution and suppression tests.
  Linux binary probes exercised help, migration, severity/exit overrides, canonical
  line/file suppressions, inert Dart ignores, malformed and unused directives.
  Corrective probes confirmed invalid UTF-8 exits 1 despite `--exit-code 5`,
  invalid usage exits 2, and syntax-error files contribute no lint baseline entries:
  repairing syntax exposed the previously uncaptured `avoid-print` finding.
  LSP incomplete-manifest replacement/recovery is covered by existing
  `server/tests.rs` and `state/tests.rs` tests, not a worker-failure binary probe.
  Review also exercised invalid-UTF-8 LSP recovery and separate workspace configs.
  Corrected config casing (including `flutter_hooks`), baseline/failure wording,
  per-workspace/member config selection and all incomplete-analysis manifest
  replacement; completed the cross-file table and compared all 21 entries,
  groups and recommended flags with registry metadata. Docgen currently owns
  only the configuration rule-options table, not this cross-file inventory.
  Documented Dart 3.13 modeling and the launcher's separate Dart 3.x requirement.
  Checks used rustc `1.101.0-nightly` (`16d1ffac1`) on x86_64 Linux.
  After corrective review, docgen/schema checks and the authorized replacement
  `numen run -- makers verify-fast` passed: 3,667 workspace tests (two manual
  timing benchmarks skipped), 18 doctests, tooling/architecture, workspace/Zed
  formatting, TOML and strict Clippy; source fingerprint
  `sha256:27ca4d11591603db5797a9f3af3377c7b224a4613c9d394f883bee0197e8aaf6`.
  Read-only published `v0.4.6` evidence: all three documented targets and both
  archive-name forms exist, with Minisign sidecars and `SHA256SUMS`; the x86_64
  archive checksum matched and its sole entry is `falcon`; `gh attestation verify`
  succeeded. Workflow source cross-builds Linux x86_64/arm64 and macOS arm64 on
  Ubuntu, gates macOS packaging on notarisation, signs archives/checksums and
  creates provenance attestations. This is not native platform execution evidence.
  Open maintainer/release decisions: select the exact candidate and supply native
  macOS/arm64 install/runtime evidence (or explicitly reduce that scope), then
  review that candidate's signatures/notarisation/provenance and approve release.
  Existing published artifacts do not certify this changed candidate; no release,
  workflow dispatch, credential change or publication was performed.
- Acceptance: every required item has source-bound evidence or an explicit
  maintainer decision reducing scope; unresolved blockers are not silently waived.

## Recommended cleanup after the enforcing slices

- [x] R1 — Replace the `LspState → ClientCapabilities → SemanticFlags →
  WorkspaceFlags` mutable `Deref` chain in `crates/falcon_lsp/src/state.rs` with
  explicit ownership/access. Keep the primary root's registries and resolution
  requirements together instead of splitting them via `into_registries` and
  storing file/cross-file flags independently; migrate tests that set those flags
  separately so they cannot contradict registered rule requirements. This is
  maintainability debt, not by itself a proven behavioral bug. Preserve
  capability/configuration lifecycle tests.
  Evidence: explicit capability/workspace fields and retained primary `ProjectAnalysis`; primary/secondary reload and capability lifecycle coverage, independent index/parser failure recovery regressions (five mutants rejected), 630 LSP/project/CLI tests, strict workspace Clippy, formatting and docgen checks passed.
- [x] R2 — Remove repeated conversions/retained data only where callers and
  profiling justify them. Keep shared immutable indexes and useful caches; no
  blanket clone/Arc ban or byte-count-based redesign.
  Evidence: LocalSend full analysis at pinned commit
  `cfd699125f2aa4c69f48fd274cf4bb1876a5e08d`, using the portable
  `bench-compare --full-analysis` configuration (all file/cross-file rules warn,
  recommended off, files `**`, four Rayon threads, no SDK or external dependency
  closure), found a repeated source-prefix scan in `max-lines-for-function`.
  Both span endpoints called `line_of`, although only their difference was
  needed. Counting newline bytes inside the span removes the quadratic
  per-file work without adding an index, cache, allocation or retained data;
  the now-unused prefix helper is removed.
  `perf record -g --call-graph dwarf` attributed 8.56% self samples to
  `check_function_lines` before and 0.04% after (15,383/14,134 samples,
  no lost samples). Other leading baseline frames were `NameMemo::get`
  (2.33%), Sip13 hashing (1.90%) and `DeclarationIdentity::hash` (1.58%).
  Stage tracing attributed 1,029 to 703 summed worker-ms to structural
  traversal, versus 1,588 to 1,636 worker-ms semantic traversal and 215 to
  217 ms cross-file rules; parallel stage sums are not wall time. Generated
  files accounted for 832 baseline structural worker-ms across 61 files,
  not one runaway file. A rule-only synthetic file with 2k/4k/8k short
  functions took 42.9/140.3/563.9 ms before and 4.5/7.7/14.0 ms after
  (three-run medians), corroborating the per-file scaling cause.
  On Linux x86_64, Ryzen 7 9800X3D (16 threads), release builds with
  rustc 1.101.0-nightly (`16d1ffac1`), 12 fresh-process runs per binary
  alternated their order on the same cached checkout. End-to-end time,
  including full JSON rendering to `/dev/null`, fell from median 1,809.8 ms
  (MAD 14.2 ms) to 1,638.4 ms (MAD 8.2 ms), a 9.47% reduction.
  This direct alternated harness is not `bench-compare`'s hyperfine lane
  (which produced the roughly 1,946–1,992 ms V1-5 baselines); the wall-time
  reduction beyond evenly divided structural worker savings remains
  unexplained.
  Sampled host one-minute load was 2.69–3.49 for both interleaved sets.
  Complete JSON output remained byte-identical: 146,823 diagnostics,
  64,147,576 bytes, SHA-256
  `ad2492f3c29619b7257f3f4a05efb7028f54b9300799e2181b4c4b30b8cf54d2`.
  LF/CRLF threshold regressions include a long Unicode-prefixed source;
  no e2e snapshots changed. Remaining semantic/hash costs are spread out,
  so no speculative cache/ownership redesign is included. This closes the
  profile-justified cleanup, not V1-5's separate Dart analyzer comparative
  acceptance item.
- [x] R3 — Consider tighter lint policies incrementally where they prevent a real
  defect. Existing pedantic/strict warnings remain mandatory. Do not spend the
  readiness phase on broad style-only churn.
  Evidence: `91ae9831da42` moved live rule filesystem access into captured
  environment facts. The `clippy` lane now denies `disallowed_methods` for
  `falcon_rules --no-deps --lib`, using `.config/clippy-rules-fs/clippy.toml` to
  cover filesystem reads/probes and analyzer discovery helpers. Production passes;
  temporary `std::fs::read_to_string` and `canonical_or_lexical` calls both fail.
  Tests retain fixture I/O without suppression; existing workspace pedantic
  warnings remain mandatory. No shipping behavior changed.
  Other candidates were measured with `cargo clippy --locked --workspace
  --message-format=json` plus `--lib --bins` (production, including the non-shipping
  `xtask`) or `--all-targets`, and `-W clippy::<lint>` for each candidate below,
  through `MBX_CACHE_LINKS=false numen run --` (2026-11-02).
  Production/all-target distinct locations: `cast_possible_truncation` 0/0
  (already pedantic); `indexing_slicing` 927/2,701, `unwrap_used` 237/7,383,
  `expect_used` 185/417 and `arithmetic_side_effects` 982/1,116 (blanket bans would
  churn guarded indexes, invariants, counters and test assertions).
  `let_underscore_must_use` 4/31 was rejected: its production hits intentionally
  discard a racing `OnceLock::set`, optional acknowledgement and two best-effort
  process-cleanup results. Counts deduplicate primary diagnostic spans by lint,
  normalized file, line and column, using `jq -s`, this file-path normalization,
  then `unique_by(.lint,.file,.line,.column)`:

  ```jq
  split("/") | reduce .[] as $p
    ([]; if $p == ".." then .[:-1] elif $p == "." then . else . + [$p] end)
    | join("/")
  ```

## Deferred unless evidence changes

Generic repository/service/DTO layers, a crate-per-layer rewrite, new task
schedulers, duplicated Numen coordination, a wholesale compiler representation
replacement, and persistent CLI analysis-result caching are not prerequisites.
Any proposal needs a concrete caller, measured problem or correctness contract,
compatibility plan and bounded acceptance criteria before implementation.
