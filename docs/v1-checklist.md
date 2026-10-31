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

- [ ] Agree and implement a shared completeness contract. Evidence:
  `crates/falcon_project/src/discovery.rs::read_source` warns/skips ordinary read
  failures; `crates/falcon_analyze/src/parallel.rs` discards companion read errors;
  `crates/falcon_lsp/src/state.rs::read_project_source` treats oversize specially.
  First slice: typed acquisition outcomes and one CLI/LSP parity fixture; then
  migrate remaining callers. Distinguish missing optional metadata from unreadable
  required source and unsupported/failed analysis. Preserve deliberate frontend
  differences explicitly.
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
  Evidence: `crates/falcon_rules/src/cross_file/secure_pubspec_urls.rs`,
  `sort_pub_dependencies.rs`, `flutter_assets.rs`, and
  `lint/support/package_manifests.rs` still acquire external facts during rule work.
  First slice: one manifest consumer and its CLI/LSP recovery test. Specify missing,
  malformed, inaccessible and changed metadata separately before migrating others.
- [ ] Expand `crates/falcon_cli/tests/project_parity_tests.rs` around actual risk:
  diagnostics identity/range/severity/message, suppression, config changes,
  incomplete analysis and cross-file invalidation. Shared stages already live in
  `falcon_project`; close specific divergence rather than invent another pipeline.
- Acceptance: a project evaluation uses coherent facts, known intentional analyzer
  differences are recorded with fixtures, and no adapter duplicates rule policy.

### V1-4 — Bound editor work and preserve cancellation

- [ ] Define request admission/backpressure/supersession for the unbounded worker
  channels/outbox in `crates/falcon_lsp/src/server.rs`. Cancellation already exists;
  preserve its transport path and measure latency before adding checkpoints.
- Acceptance: bounded retained queued work under a deterministic burst test;
  notifications required for document correctness are not dropped; cancellation,
  edit/version ordering, stale-result suppression and shutdown remain live.
  Choose capacities from measured workload evidence, not an invented guarantee.

### V1-5 — Establish representative profiling and the performance gate

- [ ] Audit `crates/falcon_rules/benches/rules_bench.rs` semantic contexts and assert
  representative semantic diagnostics before timing. Label isolated rule versus
  shared-registry costs. `benches/jfit_mobile_bench.rs` is opt-in/private and uses a
  narrower unresolved path; retain it as such, not as the release baseline.
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
