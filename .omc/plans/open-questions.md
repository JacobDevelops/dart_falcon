# Open Questions & Deferred Decisions

Track all outstanding questions that need clarification before or during execution. These items are intentionally unresolved to avoid blocking implementation.

## falcon Phase 1 (2026-06-09)

### Must Resolve Before M4 (Rule Implementation)

- [ ] **Rule specification source** — Should we reverse-engineer rule specs entirely from dart_code_linter/pyramid_lint source code, or reach out to original authors (Rainy Day, Pyramid team) for official documentation? **Why it matters:** Affects M4.0 timeline (80-100 hours); reverse-engineering carries risk of semantic mismatches.

- [x] **Semantic analysis scope for Phase 1** — **DECIDED (Iteration 2, M0.5):** Rules requiring semantic analysis (const evaluation, type inference, scope lookup) will use simplified AST-only heuristics in Phase 1, marked "simplified-v1". Examples: `no-magic-number` will ban literal numbers except 0, 1, -1 (AST heuristic); full const-eval deferred to Phase 2. Tracked in RULE_ANALYSIS_MATRIX.md (M0.5 deliverable) with semantic tags. This keeps Phase 1 focused on AST rules while documenting the simplified choices for Phase 2 improvement.

### Nice-to-Have Before M1.5 (End of Parser Phase)

- [ ] **Fuzzing strategy** — Is libFuzzer integration worth the engineering time (4-6 hours, M7 optional slot)? Or is corpus validation (M1.4, M8.1) sufficient for parser correctness? **Why it matters:** Risk mitigation vs. timeline trade-off; parser bugs are critical (R1).

- [ ] **Round-trip testing** — Should parser support pretty-print → reparse cycle (parse → AST → output → reparse → AST equality)? Or is AST snapshot testing sufficient? **Why it matters:** Affects parser design; round-trip is more robust but adds complexity.

### Optional Before M5 (LSP Integration)

- [ ] **Auto-fix scope for Phase 1** — Phase 1 explicitly excludes auto-fix. Should we identify a "safe subset" of rules with mechanical fixes (trailing-comma, format-comment) for Phase 1 quick win? **Why it matters:** Affects LSP server design; deferring to Phase 2 keeps scope tight, but quick wins boost adoption.

- [ ] **VS Code extension publishing** — Should falcon-vscode extension be published to VS Code Marketplace, or distributed only as part of jfit toolchain? **Why it matters:** Affects M5.3 scope; marketplace publishing adds ~4 hours (license file, icon, description, testing in extension host).

### Optional Before M7 (Nix Integration)

- [x] **Jfit lockfile versioning** — **DECIDED (Iteration 2):** Phase 1 uses `path:` URL for local development; Phase 2 will switch to `github:` URL with hash-pinning. Path URL allows rapid iteration; github URL with hash ensures reproducible builds in jfit CI. Recorded in plan M7.0.

### Nice-to-Have for Phase 2 Planning

- [ ] **Formatter integration** — Should Phase 2 formatter reuse AST from Phase 1 parser, or build separate formatter parser (like Prettier/Biome do)? **Why it matters:** Affects Phase 2 architecture; shared parser enables co-location of lint + format, but complicates error recovery.

- [ ] **Semantic analysis depth** — Phase 2 will likely need type inference, control flow analysis, const evaluation for comprehensive lint reimplementation (very_good_analysis rules). Should we sketch that API now (M2, analyze crate) to avoid redesign? **Why it matters:** Affects trait design in falcon_analyze; forward-looking design reduces Phase 2 refactoring.

---

## How to Use This File

1. **Before milestone start**: Review relevant open questions; escalate blockers to Planner/Architect
2. **During milestone**: If answer emerges, record decision and remove from list (with decision date)
3. **Phase 1 retrospective**: Questions not answered by end of Phase 1 → Phase 2 backlog or defer indefinitely

## Decision Log

### Iteration 2 Decisions (2026-06-09)

1. **Principle 4 Clarification** — Replaced "Streaming Diagnostics" with "Lazy AST Construction & Per-File Streaming." Core change: emit diagnostics per file (not per node), after full AST construction. This allows rules that require full AST context (member-ordering, no-equal-arguments) to function correctly while still enabling incremental LSP updates at file granularity.

2. **M0.5 Checkpoint Gate** — Added M0.5 (Trait Contracts & Grammar Scope Definition) as a mandatory checkpoint between M0 and M1. Deliverables: TRAIT_CONTRACTS.md, PARSER_GRAMMAR.md, RULE_ANALYSIS_MATRIX.md, PARALLELISM_MODEL.md. Architect approval required before M1/M2/M4 proceed. Risk mitigation: eliminates architectural uncertainty early.

3. **Semantic Analysis Scope (Phase 1)** — Rules requiring semantic analysis (const evaluation, type inference) will use simplified AST-only heuristics in Phase 1, marked "simplified-v1". Full semantic analysis deferred to Phase 2. Tracked in RULE_ANALYSIS_MATRIX.md with semantic tags (AST-only, requires-scope-lookup, requires-type-inference, requires-const-eval).

4. **M2.5 Observability Infrastructure** — Added M2.5 to establish structured logging (tracing crate) early, enabling all subsequent phases (M3-M8) to emit DEBUG logs. Includes --verbose CLI flag and structured log output format.

5. **M5.0 LSP Caching Design** — Added M5.0 to document caching strategy and incremental analysis model before M5.1 implementation. Deliverable: LSP_CACHING_DESIGN.md. Prevents mid-implementation redesign.

6. **Three Additional Pre-Mortem Scenarios** — Added Scenario 4 (LSP Correctness & Protocol Compliance), Scenario 5 (Rule Implementation Bottleneck), Scenario 6 (Test Infrastructure Failure). Mitigations: JSON-RPC 2.0 compliance tests in M5.1, rule complexity tiering in M0.5, snapshot migration scripts in M1.5.

7. **Nix Versioning Strategy** — Phase 1 uses `path:` URL for local development (rapid iteration); Phase 2 switches to `github:` URL with hash-pinning (reproducibility in jfit CI).

8. **Rule Acceptance Criteria Policy** — Defined in M4.0: same rule fires on same code (column-accurate), same message text (within 10% fuzzy match), same severity. Different suggestions/fixes acceptable in Phase 1.

9. **Timeline Update** — Total effort increased from 700-900 hours to 750-950 hours; calendar time extended from 8-10 weeks to 9-11 weeks (77 days). Added M0.5, M2.5, M5.0, and pre-mortem work.

