# jdlint Phase 1: Comprehensive Implementation Plan (Deliberate Consensus)

**Date:** 2026-06-09  
**Mode:** Deliberate Consensus (Extensive scope, full testing coverage, pre-mortem, expanded test plan, ADR)  
**Status:** PENDING APPROVAL — Planner/Architect/Critic consensus achieved (2026-06-09, Iteration 3)  
**Previous Status:** Iteration 3 - Critic execution readiness gaps resolved (2026-06-09)

---

## 1. RALPLAN-DR Summary

### 1.1 Principles (7 Guiding Technical Principles)

1. **Hand-Rolled Parser Trust**: No external dependencies on tree-sitter or Dart SDK. A correct Dart 3.x recursive descent parser is the foundation; every diagnostic depends on it.
2. **Parallel-First Architecture**: Rayon-based work stealing for multi-file analysis. Single-file analysis must be fast enough; parallelism handles corpus scale (<1s on 214 files).
3. **Biome-Inspired Modular Crates**: Each semantic concern in its own crate with clear trait boundaries. Rule visitor trait allows composable, testable rule implementations.
4. **Lazy AST Construction & Per-File Streaming**: Build AST fully before rule analysis (not per-node streaming), then emit diagnostics per file as that file finishes analysis. This enables incremental LSP updates at file granularity without requiring node-level streaming, which would conflict with rules that require full AST context (member-ordering, no-equal-arguments, etc.). Architecture: parse all files → analyze files in parallel via Rayon → emit diagnostics per file as done.
5. **Explicit Rule Ports, Not Reimplementation**: Port only enabled rules from jfit's analysis_options.yaml. No reimplementation of dart analyze or very_good_analysis built-ins.
6. **Config-as-Contract**: jdlint.json is the contract; each field and its default must be documented and testable. No magic or implicit behavior.
7. **Observable System Health**: Metrics, logging, and benchmarks are first-class. <1s acceptance criterion is measurable via `cargo bench`.

### 1.2 Decision Drivers (Top 5)

| Driver | Impact | Constraint |
|--------|--------|-----------|
| **Phase 1 scoping (rules only, no format/check)** | Reduces complexity by 60%; enables fast iteration on parsing & rule porting. | Must defer formatter & comprehensive lint reimplementation to Phase 2. |
| **Hand-rolled parser (no tree-sitter)** | Full Dart 3.x semantic context; avoids tree-sitter plugin cost & AST mismatch. | Parser bug → diagnostic cascades; requires rigorous corpus testing. |
| **Biome-style modular workspace (94 crates as reference)** | Decouples parser, rules, LSP, CLI; enables parallel development by multiple agents. | Workspace management, version pinning, inter-crate testing complexity. |
| **Rayon parallelism (Biome reference)** | <1s on 214-file jfit mobile project; leverages modern CPU scaling. | Requires thread-safe rule state; no mutable shared diagnostics across workers. |
| **Nix flake + jfit integration** | Hermetic, reproducible builds; jfit can lock version and deploy with confidence. | Must sync with jfit's flake.nix; changes to flake or dependencies ripple. |

### 1.3 Viable Options (≥2 Major Approaches)

#### Option A: **Monolithic Crate + Single Binary** (REJECTED)
- All parsing, rules, LSP, CLI in one `src/` tree
- **Pros**: Simpler initial setup; faster to first lint result
- **Cons**: Untestable rule logic in isolation; LSP logic entangled with parser; 10x harder to parallelize rule execution; harder for agents to work independently
- **Why Rejected**: Violates Principle 3 (modular crates); blocks parallel development; testing becomes intractable at scale

#### Option B: **Full Biome Clone (Workspace with 90+ Crates)** (REJECTED)
- Replicate Biome's exact structure (biome_deserialize, biome_diagnostics_macros, biome_control_flow, etc.)
- **Pros**: Proven architecture; can reuse snippets from Biome
- **Cons**: Massive over-engineering for Dart (single language); bloated CI/CD; version coordination overhead; excessive boilerplate
- **Why Rejected**: Dart is monolingual; Biome multi-language complexity not needed; would extend timeline by 2-3x

#### **Option C: Biome-Inspired Modular Workspace (8-10 Core Crates)** [SELECTED]
- **Crates**: jdlint_dart_parser, jdlint_syntax, jdlint_analyze, jdlint_rules, jdlint_diagnostics, jdlint_lsp, jdlint_config, jdlint_cli, jdlint (binary)
- **Macros crate** (optional): jdlint_analyze_macros if boilerplate becomes severe
- **Xtask**: codegen for rule boilerplate (visitor trait impl, test stubs)
- **Pros**: 
  - Clear separation of concerns; 4-6 agents can work on different crates in parallel
  - Parser logic isolated; rules can be tested without full LSP stack
  - CLI, LSP, config are independent; changes don't cascade
  - Scale to 50-80 rules without monolithic growth
  - Proven by Biome (fewer crates, same discipline)
- **Cons**: 
  - More integration points to test
  - Inter-crate version compatibility (mitigated by workspace members)
  - Agents must understand trait boundaries
- **Why Selected**: Best risk/reward for Phase 1 timeline (6-8 weeks estimated); enables parallel development; Biome-proven; accommodates future Phase 2 expansion without rework

---

## 2. Pre-Mortem (Deliberate Mode – 3 Failure Scenarios)

### Scenario 1: Parser Correctness Crisis (Probability: MEDIUM | Impact: CRITICAL)

**What goes wrong:**
- Hand-rolled Dart 3.x parser has subtle bugs (null-coalescing, records, sealed classes, patterns) that surface only on real jfit code
- Diagnostics upstream of parser bugs are silently incorrect or missing
- Discovered late in testing phase (week 5-6) when running on full jfit corpus

**Why it fails:**
- Dart grammar is complex; edge cases (operator precedence, pattern matching semantics) are easy to miss
- No reference implementation (tree-sitter) to validate against
- Testing starts with synthetic fixtures; real code has corner cases (multiline strings, deep nesting, complex type parameters)

**Mitigation strategy:**
- **Immediate (M1)**: Grammar reference document (8 hours) — transcribe Dart spec sections relevant to parser
- **Parallel (M2-M3)**: Corpus-driven TDD — write parser rules alongside jfit .dart corpus tests, not after
- **Checkpoint (end M3)**: Full parse of jfit mobile lib without errors; run corpus snapshot tests
- **Continuous**: Fuzzing (optional, but recommended for week 7) — libFuzzer on parser with jfit corpus seeds
- **Recovery**: If parser bugs found after M3, fork and fix in parallel; delay other rules if needed
- **Owner**: Parser engineer (agent 1)

### Scenario 2: Rule Porting Semantics Mismatch (Probability: MEDIUM | Impact: HIGH)

**What goes wrong:**
- Ported `no-magic-number` rule produces different diagnostics than `dart_code_linter` on same code
- Threshold is off (e.g., `0` and `1` should be exempt, but aren't)
- Root cause: rule spec is ambiguous; dart_code_linter implementation has undocumented heuristics

**Why it fails:**
- dart_code_linter and pyramid_lint documentation is sparse
- Behavior must be reverse-engineered from source and test output
- No golden test corpus from dart_code_linter to validate against
- Ambiguous specs (e.g., "prefer-trailing-comma" — when exactly?) lead to interpretation divergence

**Mitigation strategy:**
- **M4**: Rule specification audit (one rule per engineer) — read source, run on test code, document behavior
- **M5**: Golden test corpus — for each ported rule, capture 5-10 jfit code snippets + expected diagnostics from original linter
- **M6**: Diff-based rule validation — run jdlint and dart_code_linter side-by-side on corpus; flag mismatches
- **Checkpoint (end M6)**: Zero mismatches on golden corpus for all rules
- **Continuous**: Code review focus on rule semantics (architect + original rule author if available)
- **Recovery**: If mismatches found, re-engineer rule with spec correction, re-test corpus
- **Owner**: Rule engineers (agents 2-3); rule spec author for design clarification

### Scenario 3: Performance Regression / Failure to Meet <1s Criterion (Probability: LOW | Impact: HIGH)

**What goes wrong:**
- Parallelism overhead exceeds gains; single-threaded parse + analyze is faster than Rayon multi-threaded version
- Or, rule implementations are inefficient (e.g., deep AST clones, n² visitor passes)
- Full jfit corpus takes 2-3 seconds; acceptance criterion fails

**Why it fails:**
- Rayon context switch cost + rule allocation overhead not measured until late (M8)
- Early benchmarking skipped; micro-optimizations deferred
- Parser or rule logic has hidden complexity (e.g., string interning not implemented, regex compiled per file)

**Mitigation strategy:**
- **M3**: Benchmark harness (2 hours) — `cargo bench` on 50-file sample; track parse time, rule time, total
- **M5**: Rule microbenchmarks — each rule has bench target; track per-rule overhead
- **M7**: Full corpus profiling — `perf` profile jdlint on jfit mobile; identify bottlenecks early
- **M8**: Optimization sprint — if >1.2s, profile & optimize (string interning, rule parallelization, AST compression)
- **Checkpoint (end M8)**: Full jfit corpus <1s on reference hardware (Linux x86_64, modern CPU)
- **Recovery**: If >1.2s and analysis shows parser not bottleneck, split corpus into per-module passes (recovery mode, not final design)
- **Owner**: Performance engineer (agent dedicated to M7-M8)

### Scenario 4: LSP Correctness & Protocol Compliance (Probability: MEDIUM | Impact: HIGH)

**What goes wrong:**
- LSP server sends malformed JSON-RPC 2.0 responses; editor (VS Code) shows spurious errors or drops diagnostics silently
- Edge case: VS Code receives `PublishDiagnostics` with wrong file URI format; diagnostics appear in wrong file
- Root cause: LSP message construction doesn't validate against spec; mismatch between `lsp-server` crate version and LSP 3.17 spec

**Why it fails:**
- JSON-RPC 2.0 is strict; invalid request IDs, missing fields, or malformed types cause silent drops
- LSP 3.17 spec is 100+ pages; easy to miss required fields (e.g., `capabilities` in initialize response)
- Testing only happens in M5.3 (VS Code manual testing); earlier issues not caught
- No automated compliance checking against lsp_types crate

**Mitigation strategy:**
- **M5.0 (early)**: Review LSP 3.17 spec (2 hours); document expected message shapes for initialize, open, change, save, publishDiagnostics
- **M5.1**: LSP foundation tests validate JSON-RPC 2.0 format using `serde_json::from_str` against `lsp_types::` types
- **M5.1**: Unit test: send mock client request; verify response shape matches `InitializeResult`, `PublishDiagnosticsParams`, etc.
- **M5.3**: VS Code extension smoke test; capture and validate JSON-RPC traces
- **Checkpoint (M5.4)**: Validate 10 message types against `lsp-types` crate; zero malformed messages
- **Owner**: LSP Engineer (Agent 4); Architect review of LSP message construction

### Scenario 5: Rule Implementation Bottleneck (Probability: MEDIUM | Impact: MEDIUM)

**What goes wrong:**
- 60 rules is a large scope; one or two complex rules (member-ordering, no-magic-number with const eval) take 3x estimated time
- Rule implementation bogs down in M4.2-M4.6; parallel agents waiting for framework clarification
- Timeline slips by 1-2 weeks; LSP and integration phases compressed

**Why it fails:**
- Complexity estimates (2-3 hours per rule) assume straightforward AST pattern matching
- Some rules require deeper context: member-ordering needs class member sequence analysis; no-magic-number requires const evaluation
- No pre-implementation complexity tiering; rules of similar complexity grouped together, causing bottlenecks

**Mitigation strategy:**
- **M0.5**: Complexity audit — categorize 60 rules as SIMPLE (1-2h), MEDIUM (2-3h), COMPLEX (4-6h) before implementation
- **M0.5**: Identify top 3 complex rules; pre-plan their implementation strategy
- **M4.0**: Separate rule implementation tracks: SIMPLE rules in M4.2-M4.4 (parallel, 6 agents), COMPLEX rules in M4.5-M4.6 (2-3 dedicated agents)
- **M4.1**: Framework + codegen tested on both SIMPLE and COMPLEX rule stubs; ensure framework doesn't block complex rules
- **Checkpoint (M4.3 midpoint)**: Review progress on complex rules; reallocate agents if slipping
- **Recovery**: If complex rules slip >50%, defer non-critical rules to Phase 2 "simplified" versions with AST-only heuristics
- **Owner**: Rule Engineers (Agents 2-6); Architect guidance on complexity tiers

### Scenario 6: Test Infrastructure Failure (Probability: LOW | Impact: MEDIUM)

**What goes wrong:**
- Parser AST format changes after M1.5 (e.g., refactoring node enum, renaming fields)
- Insta snapshot tests fail en masse; 200+ golden corpus files (.dart + snapshots) require manual review
- Test maintenance overhead explodes; snapshot updates take 4-6 hours per change

**Why it fails:**
- Snapshots are brittle; any AST format change invalidates all snapshots
- No migration strategy for snapshot updates
- Golden corpus tests (M4, M8) are snapshot-heavy; format changes ripple through entire test suite

**Mitigation strategy:**
- **M1.5**: Lock parser AST format (declare format "final" for Phase 1); document format in comments
- **M1.5**: Create migration script: `scripts/migrate_snapshots.sh` for bulk snapshot updates
- **M2-M8**: If AST format change needed, use migration script + one-shot review (less time than manual per-snapshot updates)
- **M4.1**: Golden corpus test framework uses insta with `--auto` review mode (human review once, auto-approve subsequent runs)
- **M8.1**: Before committing golden corpus snapshots, verify migration strategy is in place
- **Recovery**: If migration script fails, revert format change and defer redesign to Phase 2
- **Owner**: QA Engineer (Agent 9); Build Engineer (version control, migration strategy)

---

## 3. Implementation Milestones (M0-M9)

### M0: Project Setup & Infrastructure (Week 1, Days 1-2) — Complexity: **SMALL**

**What gets built:**
1. Rename repo from `jlint` to `jdlint`; update Cargo.toml `name = "jdlint"`
2. Scaffold workspace Cargo.toml with 9 crates listed
3. Create flake.nix for jdlint with Rust stable toolchain + build target
4. Setup GitHub Actions CI: lint, test, bench (Linux x86_64 only for Phase 1)
5. Create `.omc/plans/jdlint-rules-matrix.md` (rule-to-crate mapping)
6. Establish dev environment: nix flake (test `nix develop` produces jdlint binary)

**Tests written:**
- CI pipeline functional tests (yaml syntax, checkout, build steps)
- flake.nix evaluates without errors
- `cargo build --release` produces zero warnings (on stable Rust)

**Exit criteria:**
- Repo renamed to jdlint; all references updated in git history
- `nix flake update` runs clean
- CI pipeline triggers on push; `nix develop` makes rustc, cargo available
- Workspace builds: `cargo build --workspace`
- All 9 crates listed in Cargo.toml; each has stub src/lib.rs

**Estimated effort:** 4-6 hours (1 agent)

---

### M0.5: Trait Contracts & Grammar Scope Definition (Week 1-2, Days 2-3) — Complexity: **MEDIUM**

**Purpose:** Define trait APIs, parser grammar scope, rule analysis matrix, and parallelism model BEFORE implementation begins. These are load-bearing architectural artifacts required by M1, M2, and M4.

#### M0.5.1: Trait Contracts Document

**What gets built:**
1. `.omc/docs/TRAIT_CONTRACTS.md` — Rust trait specifications for Rule, Visitor, AnalyzeContext
   - `Rule` trait: `name(&self) -> &'static str`, `analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>`
   - `RuleVisitor` trait: extends Visitor<Vec<Diagnostic>>; methods for each node type
   - `AnalyzeContext`: config, file_path, source code, diagnostic sink
   - Concrete end-to-end example: implementing `avoid_dynamic` rule from scratch
   - Thread safety guarantees: rule instances are immutable; diagnostics are local per file
2. Review gate: Architect approval required before M1.2/M2.2 begin

**Tests written:**
- Compile-time check: trait methods compile; example rule is valid

**Exit criteria:**
- Trait contracts are unambiguous; rule engineers can implement without clarification
- Example rule implementation is complete and runnable
- Architect has approved design

**Estimated effort:** 8-10 hours (1 agent + Architect review)

---

#### M0.5.2: Parser Grammar Scope Document

**What gets built:**
1. `.omc/docs/PARSER_GRAMMAR.md` — Dart 3.x grammar scope for jdlint Phase 1
   - Full Dart 3.x features explicitly supported: null-coalescing (?.), records, patterns, sealed classes, extensions, mixins, enums
   - Explicitly non-scope: Dart 2.x-only syntax (no longer relevant)
   - Grammar reference (produced in M1.1 as `grammar.md`)
   - Scope boundaries: which productions are required for Phase 1 rules
   - Edge cases documented: deeply nested type parameters, complex pattern matching, function type syntax

**Tests written:**
- Validation: scope document is complete before M1.2 parser implementation

**Exit criteria:**
- Parser grammar scope is clear; M1.2 implementer knows what to parse and what to defer
- Architect has reviewed scope for completeness

**Estimated effort:** 4-6 hours (1 agent + Architect review)

---

#### M0.5.3: Rule Analysis Matrix

**What gets built:**
1. `.omc/docs/RULE_ANALYSIS_MATRIX.md` — Complexity and semantic analysis requirements for all 60 rules
   - Table: rule name, complexity tier (SIMPLE/MEDIUM/COMPLEX), category, semantic requirements
   - Semantic tags: `AST-only` | `requires-scope-lookup` | `requires-type-inference` | `requires-const-eval`
   - Phase 1 decision: rules tagged with semantic requirements will use simplified heuristics; complex rules deferred or marked "simplified-v1"
   - Example: `no-magic-number` = COMPLEX, requires-const-eval; Phase 1 uses simplified heuristic (ban literal numbers except 0, 1, -1)

**Tests written:**
- Validation: all 60 rules categorized; no gaps

**Exit criteria:**
- Rule implementation complexity is predictable; resource allocation (agents, hours) is accurate
- Architect approves complexity tiers
- Semantic scope is clear for rule implementation

**Estimated effort:** 6-8 hours (1 agent, with rule spec input from M4.0 prep)

---

#### M0.5.4: Parallelism Model Document

**What gets built:**
1. `.omc/docs/PARALLELISM_MODEL.md` — Rayon parallelization strategy for jdlint
   - Decision: per-file parallelism (recommended) vs. per-rule parallelism
   - **Per-File Model (Selected for Phase 1):** Each .dart file is one Rayon work unit; all rules run sequentially on that file within the task. Rationale: file-level parallelism is sufficient for 214-file jfit corpus; per-rule adds complexity without benefit unless single-file analysis exceeds 100ms
   - Architecture: `RuleRegistry::analyze_parallel(files, rules, config)` → Rayon scope per file → sequential rule execution within file
   - Baseline measurement plan: M1.5 benchmark (50-file sample); if per-file single-threaded >100ms, revisit per-rule strategy in M6
   - Thread safety: each file has isolated diagnostics vector; no shared mutable state

**Contingency: Per-Rule Parallelism Switch**
If M6.2 profiling shows single-file analysis consistently exceeds 100ms (due to uneven 
file size distribution in jfit corpus):
- Switch to per-rule parallelism: rules run in parallel on same file via Rayon ThreadPool
- Requires AnalyzeContext.diag_sink to become thread-safe: `Arc<Mutex<Vec<Diagnostic>>>`
- **Preemptive design requirement**: RuleVisitor implementations MUST NOT use mutable self 
  state; all state must be immutable or thread-local (enforced at M2.2 code review)
- Trigger condition: median single-file analysis time >100ms at M1.5 baseline measurement
- Decision gate: M6.2 profiling report reviewed by Architect before switching

**Tests written:**
- Validation: parallelism model is consistent with AnalyzeContext design

**Exit criteria:**
- Parallelism model is clear for M2 (analyze crate) and M6 (optimization) phases
- Architect approves strategy

**Estimated effort:** 4-6 hours (1 agent + performance engineer input)

---

#### M0.5 Checkpoint & Gate

**Activities:**
1. Complete all 4 documents: TRAIT_CONTRACTS, PARSER_GRAMMAR, RULE_ANALYSIS_MATRIX, PARALLELISM_MODEL
2. Architect review: all documents are approved (signatures/sign-off in plan comments)
3. Rule spec prep (concurrent): prep M4.0 rule specifications for all 60 rules

**Exit criteria (gating M1, M2, M4):**
- All trait contracts specified and approved
- Parser grammar scope locked
- Rule complexity tiers assigned
- Parallelism model decided and documented

**M0.5 Gate Enforcement:**
- All 4 design documents (TRAIT_CONTRACTS.md, PARSER_GRAMMAR.md, RULE_ANALYSIS_MATRIX.md, PARALLELISM_MODEL.md) must be committed to `.omc/docs/` before M1.1 branch is created
- Architect approval documented in `.omc/plans/M0.5-SIGN-OFF.txt`:
  * Format: Date: YYYY-MM-DD | Architect: [Name] | Decision: APPROVED | APPROVED-WITH-REWORK
  * If APPROVED-WITH-REWORK: max 2-day turnaround for fixes before re-review
- CI gate: M1 branch merge is blocked if `.omc/plans/M0.5-SIGN-OFF.txt` is missing or unsigned
- M1 branch is not created until sign-off file exists

**Estimated effort:** 28-36 hours (1 main agent + Architect + optional rule spec prep)

---

### M0.6: Pre-Flight Rule Spec Audit (Week 1, Day 3) — Complexity: SMALL

**What gets built:**
1. Fetch dart_code_linter ^3.2.1 source from pub.dev or GitHub
2. Fetch pyramid_lint ^2.4.0 source from pub.dev or GitHub
3. Cross-reference each source with the 60 jfit-enabled rules (34 dart_code_linter + 26 pyramid_lint)
4. Create `.omc/docs/RULE_AUDIT_SOURCES.md` documenting:
   - Source repo URL, branch/tag, commit hash for each package
   - Version pin used (dart_code_linter ^3.2.1, pyramid_lint ^2.4.0)
   - List of all 60 rules with source file locations (e.g., `lib/src/rules/avoid_dynamic.dart`)
   - Any rules unavailable or ambiguous in source → escalate to TBD process (M4.0.6)
5. Verify source accessibility for all 60 rules before M4.0 audit begins

**Acceptance:**
- RULE_AUDIT_SOURCES.md committed to `.omc/docs/`
- All 60 rules have located source files (or escalated to TBD)
- Source version pins documented
- No blockers for M4.0 spec audit

**Estimated effort:** 2-4 hours (1 agent)

**Why this matters:** M4.0 allocates 86-108 hours for rule spec reverse-engineering. If source is inaccessible or version-mismatched, that estimate collapses. M0.6 eliminates this risk at <4 hours cost.

---

### M1: Hand-Rolled Dart 3.x Parser (M1.1-M1.5) (Weeks 2-3, Days 4-11) — Complexity: **LARGE**

#### M1.1: Grammar & Lexer (Days 4-5)

**What gets built:**
1. `jdlint_dart_parser/src/lexer.rs` — token stream for Dart 3.x
   - Tokens: IDENTIFIER, NUMBER (int/double), STRING (single/double/raw/multiline), KEYWORD, OPERATOR, PUNCT, WHITESPACE, COMMENT
   - Keywords: class, abstract, interface, sealed, final, const, var, dynamic, void, async, await, yield, enum, mixin, extension, etc.
   - Support Dart 3.x patterns: `records` (parentheses), `patterns` (match), `sealed classes`
   - Multiline string handling (```, """)
   - String interpolation escape ($identifier, ${expression})
   - Comment types: //, /*, */, ///
2. `jdlint_dart_parser/src/grammar.md` — Dart grammar reference (transcribed from spec)
   - Scope: full grammar; mark rules used in Phase 1 parsing
   - Include productions for: compilation_unit, class, function, expression, statement, type, pattern
3. Test suite: `jdlint_dart_parser/tests/lexer_tests.rs`
   - Lex 50 hand-crafted Dart snippets (keywords, operators, strings, comments)
   - Snapshot tests for token stream (insta crate)

**Tests written:**
- Unit: lexer.rs — tokenize each keyword, operator, string variant
- Integration: lex real jfit code sample; snapshot token output
- Corpus: 5 jfit .dart files; verify token stream is complete

**Exit criteria:**
- Lexer produces correct tokens for all Dart 3.x token types
- No panics on malformed input (e.g., unclosed strings); emit error tokens instead
- 50 lex unit tests + 5 corpus tests pass
- Grammar reference complete for parser implementation (M1.2)

**Estimated effort:** 8-12 hours (1 agent)

---

#### M1.2: Recursive Descent Parser Core (Days 6-9)

**What gets built:**
1. `jdlint_dart_parser/src/parser.rs` — recursive descent parser
   - Core productions: `compilation_unit`, `import_directive`, `class_declaration`, `function_declaration`, `method_declaration`
   - Expression parsing with precedence climbing: assignments, ternary, logical OR/AND, comparison, equality, additive, multiplicative, unary, postfix, primary
   - Type parsing: `type`, `type_parameter`, `function_type`, `type_arguments`
   - Statement parsing: `block`, `if`, `while`, `for`, `do-while`, `switch`, `try-catch`, `return`, `throw`, `expression_statement`
   - Pattern parsing (Dart 3.x): `pattern`, `list_pattern`, `record_pattern`, `map_pattern`, `variable_pattern`, `wildcard_pattern`
   - Error recovery: skip to statement boundary on parse error; emit error node instead of panic
2. Parser tests: `jdlint_dart_parser/tests/parser_tests.rs`
   - 100+ hand-crafted test cases covering all productions
   - Round-trip tests: parse → pretty-print → reparse (optional for Phase 1)

**Tests written:**
- Unit: Each production has 3-5 test cases (valid, edge case, error recovery)
- Integration: Parse 20-file jfit corpus; snapshot AST output; verify no panics
- Snapshot tests (insta): parser output for complex expressions, class hierarchies, sealed classes

**Exit criteria:**
- All 100 parser unit tests pass
- jfit mobile lib parses to completion (214 files, no panics)
- AST structure matches grammar reference
- Error messages are recoverable (no cascading failures)

**Estimated effort:** 40-60 hours (1 parser specialist agent)

---

#### M1.3: AST & Syntax Crate (Days 6-9, parallel with M1.2)

**What gets built:**
1. `jdlint_syntax/src/lib.rs` — AST node definitions
   - Use enum-based AST (like Biome), not trait objects
   - Core nodes: `Program`, `ClassDecl`, `FunctionDecl`, `MethodDecl`, `ExprStmt`, `IfStmt`, `Block`, `Expression`, `Pattern`, etc.
   - Span tracking: each node has byte range (start, end) for diagnostic attribution
   - SyntaxKind enum: classifies nodes for visitor pattern
2. `jdlint_syntax/src/visitor.rs` — Visitor trait for AST traversal
   - Trait `Visitor<R>` with methods: `visit_program`, `visit_class`, `visit_function`, `visit_expression`, etc.
   - Default visitor walks full tree; rules override specific methods
   - Return type R allows rules to accumulate diagnostics
3. Tests: `jdlint_syntax/tests/ast_tests.rs`
   - Create AST nodes directly; verify structure
   - Visitor pattern tests: traverse sample AST; verify all nodes visited

**Tests written:**
- Unit: AST node construction, span tracking
- Integration: Visitor walks full program AST; collects all node kinds

**Exit criteria:**
- All AST nodes compile; no compilation errors
- Visitor trait is used in M2 (analyze crate); verify trait is ergonomic
- Span tracking verified on parsed jfit code

**Estimated effort:** 12-16 hours (1 agent)

---

#### M1.4: Parser Integration Tests (Days 10-11)

**What gets built:**
1. Corpus test suite: `jdlint_dart_parser/tests/corpus_tests.rs`
   - Compile jfit mobile lib (214 files) via jdlint parser
   - Snapshot parse tree for each file (JSON or custom format)
   - Verify no panics, no parse errors on valid Dart code
2. Round-trip tests (optional): parse → output AST JSON → verify structure

**Tests written:**
- Corpus: All 214 jfit .dart files
- Snapshot: One snapshot per file; commit to git
- Regression: any parse error blocks merge

**Exit criteria:**
- 100% of jfit mobile lib parses without panic
- Zero parse errors on valid Dart code
- Snapshots committed; any parser change requires snapshot review

**Estimated effort:** 4-8 hours (1 agent)

---

#### M1.5: Parser Checkpoint & Review (End of Week 3)

**Step M1.5.1: AST Format Specification**
- File: `crates/jdlint_syntax/src/FORMAT.md`
- Content:
  - Complete Rust type definitions (copied from AST enum at time of lock)
  - JSON schema for serialized AST (used by corpus snapshot tests)
  - Versioning: semantic (MAJOR.MINOR); Phase 1 locks to MAJOR=1
  - Breaking vs. non-breaking changes:
    * Breaking: enum variant removal, field type change, struct member removal
    * Non-breaking: new enum variant, new optional field, enum variant reorder
  - Lock constant: `const JDLINT_AST_FORMAT_VERSION: &str = "1.0"` in jdlint_syntax crate
- Acceptance:
  - FORMAT.md committed and reviewed by Architect
  - TRAIT_CONTRACTS approval gate (M0.5) includes AST format review
  - `JDLINT_AST_FORMAT_VERSION` constant present in jdlint_syntax/src/lib.rs
  - Any breaking AST change after M1.5 requires Architect approval and snapshot migration
- Estimated effort: 2-4 hours (1 agent)

**Activities:**
- Code review: parser logic, error recovery, visitor interface
- Performance baseline: `cargo bench` on 50-file sample; record parse time
- Corpus verification: spot-check 20 snapshots for correctness
- AST format locked: FORMAT.md committed and reviewed

**Exit criteria (gating M2):**
- All parser tests pass
- Performance <100ms for 50 files (single-threaded baseline)
- FORMAT.md committed and reviewed; AST format locked for Phase 1
- Architect approves parser design & quality

---

### M2: Diagnostics & Core Analyze Infrastructure (Weeks 3-4, Days 11-16) — Complexity: **LARGE**

#### M2.1: Diagnostics Types & Serialization (Days 11-12)

**What gets built:**
1. `jdlint_diagnostics/src/lib.rs`
   - Types: `Diagnostic`, `Severity` (Error, Warning, Info, Note), `Span` (start, end, file)
   - Methods: `with_message()`, `with_code()`, `with_suggestion()`, `with_context_lines()`
   - Serialize to: JSON (for CLI output), LSP protocol format (for editor), text (for console)
2. Tests: `jdlint_diagnostics/tests/diag_tests.rs`
   - Create diagnostics; verify serialization to JSON, LSP, text
   - Snapshot tests for output formats

**Tests written:**
- Unit: diagnostic construction, formatting, serialization
- Integration: diagnostic collection and output pipeline

**Exit criteria:**
- Diagnostic types fully specified
- JSON serialization verified
- LSP serialization matches Language Server Protocol spec (reviewed against JSON-RPC schema)

**Estimated effort:** 6-8 hours (1 agent)

---

#### M2.2: Analyze & Rule Traits (Days 12-15)

**What gets built:**
1. `jdlint_analyze/src/lib.rs`
   - Core trait `Rule`: `fn analyze(&self, tree: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>`
   - Trait `RuleVisitor`: extends Visitor<Vec<Diagnostic>>; visits nodes and collects diagnostics
   - Struct `AnalyzeContext`: config, file path, source code, diagnostic collector
   - Trait `RuleRegistry`: register rules by name; lookup and run by config
   - Parallel execution: `analyze_parallel(rules, files, config) -> Vec<Diagnostic>` using Rayon
2. `jdlint_analyze/src/visitor_macros.rs` (optional): derive macro for common visitor patterns
3. Tests: `jdlint_analyze/tests/analyze_tests.rs`
   - Mock rule implementation; verify visitor pattern
   - Parallel analysis on 10-file sample; verify thread safety

**Tests written:**
- Unit: rule trait, visitor contract, context passing
- Integration: mock rule on sample AST; diagnostics collected correctly
- Concurrency: parallel analysis on multi-file input; no data races (miri linter)

**Exit criteria:**
- Rule trait ergonomic (agents can implement rules easily)
- Visitor pattern verified end-to-end
- Parallel execution tested (10 files, no races, correct diagnostics)
- AnalyzeContext fully specified for rule access

**Estimated effort:** 20-24 hours (1-2 agents)

---

#### M2.3: Config Deserialization (Days 13-14)

**What gets built:**
1. `jdlint_config/src/lib.rs`
   - Struct `JdlintConfig`: loaded from jdlint.json
   - Fields: `rules` (Map<String, RuleConfig>), `exclude_patterns`, `severity_override`, `max_errors`
   - Each rule: `RuleConfig { enabled: bool, options: Map<String, Value> }`
   - Defaults: default jdlint.json with all Phase 1 rules enabled
   - Validation: missing rules, invalid field names → warnings, not errors
2. Serde integration: `serde_json` deserialization with custom error messages
3. Tests: `jdlint_config/tests/config_tests.rs`
   - Load valid jdlint.json; verify parsing
   - Load invalid JSON; verify error messages
   - Load partial config (missing rules); verify defaults applied
   - Snapshot tests for config structure

**Config File Discovery (Priority Order):**
1. CLI flag: `jdlint check --config /path/to/custom.json` (explicit override)
2. Current working directory: `./jdlint.json`
3. Git root directory: `<git-root>/jdlint.json` (walk parent dirs until .git found)
4. User home directory: `$HOME/.jdlint.json`
5. Hardcoded defaults: all Phase 1 rules enabled, no exclude patterns

**Tests:**
- Unit: verify each priority level is respected (--config flag wins, home dir is last resort)
- Integration: test with no jdlint.json present (defaults applied correctly)

**Tests written:**
- Unit: deserialize valid, invalid, partial configs
- Integration: real jdlint.json files; verify fields
- Snapshot: config deserialization output

**Exit criteria:**
- jdlint.json fully specified and validated
- All Phase 1 rules have config entries (default enabled)
- Config API is clean for CLI/LSP to consume
- Config discovery order is documented and tested

**Estimated effort:** 8-12 hours (1 agent)

---

#### M2.4: Observability Infrastructure (Days 15-16)

**What gets built:**
1. `jdlint_tracing/src/lib.rs` or integration with `tracing` crate (optional separate crate)
   - Structured logging via `tracing` crate (log, span, event macros)
   - Log levels: TRACE (per-token), DEBUG (per-node, rule execution), INFO (file start/end, analysis summary), WARN (unexpected conditions), ERROR (failures)
   - Per-component log levels: parser (DEBUG), analyze (DEBUG), LSP (DEBUG), CLI (INFO)
   - Span hierarchy: span per file → span per rule → events per violation detected
2. CLI flag: `--verbose` (enables DEBUG logs; default INFO)
   - Examples: `jdlint check . --verbose` shows detailed analysis traces
   - Machine-readable: optional `--log-format json` for structured log output
3. Metrics instrumentation (foundations, not collection yet):
   - Track: files processed, diagnostics per file, per-rule execution time (via span durations)
   - Metrics available in M6 benchmarking
4. Tests: `jdlint_cli/tests/logging_tests.rs`
   - Verify logs are emitted at correct levels
   - Verify `--verbose` flag enables DEBUG logs
   - Snapshot: log output format

**Tests written:**
- Unit: log level configuration, span hierarchy
- Integration: run CLI with --verbose; verify DEBUG logs present

**Exit criteria:**
- Logging infrastructure in place across all crates
- All major operations (parse, analyze, rules) emit DEBUG logs
- CLI respects --verbose flag
- Log format is structured and machine-readable (JSON optional, text with spans OK)

**Estimated effort:** 6-8 hours (1 agent)

---

#### M2.5: Analyze & Infrastructure Checkpoint (End of Week 4)

**Activities:**
- Code review: traits, config design, parallel infrastructure
- Integration test: end-to-end analyze on 10-file jfit sample
- Design review: rule interface is ready for Phase 2 (M4)

**Exit criteria (gating M3-M4):**
- All analyze unit tests pass
- Parallel execution verified safe
- Config API approved by team

---

### M3: CLI Framework & Integration (Weeks 4-5, Days 17-23) — Complexity: **MEDIUM**

#### M3.1: CLI Skeleton with Clap (Days 17-18)

**What gets built:**
1. `jdlint_cli/src/lib.rs` — CLI argument parsing
   - Subcommands: `check` (lint all files), `lsp` (start LSP server), `version`
   - Flags: `--config <path>`, `--exclude <glob>`, `--max-errors <n>`, `--format json|text`, `--quiet`, `--exit-code <n>`
   - Clap derive macros for ergonomic parsing
2. `jdlint/src/main.rs` — binary entrypoint
   - Parse args, load config, run selected subcommand
   - Exit with appropriate code
3. Tests: `jdlint_cli/tests/cli_tests.rs`
   - Mock file system; test arg parsing
   - Verify exit codes

**Tests written:**
- Unit: clap parsing, arg validation
- Integration: invoke jdlint binary with test args; verify output format

**Exit criteria:**
- `jdlint --help` and `jdlint check --help` work
- `jdlint --version` outputs semver
- Config file loading works

**Estimated effort:** 6-8 hours (1 agent)

---

#### M3.2: File Discovery & Walk (Days 18-20)

**What gets built:**
1. `jdlint_cli/src/file_walker.rs`
   - Walk directory tree; find .dart files
   - Respect exclude patterns (from config)
   - Return Vec<(Path, SourceCode)>
   - Handle symlinks, permissions errors gracefully
2. Tests: `jdlint_cli/tests/walker_tests.rs`
   - Create temp directory with .dart files; walk and verify
   - Test exclude patterns (glob syntax)

**Tests written:**
- Unit: file walker on synthetic directory tree
- Integration: walk jfit mobile lib; verify all 214 files found

**Exit criteria:**
- File discovery works on jfit mobile lib (214 files)
- Exclude patterns respected
- No panics on permission errors

**Estimated effort:** 4-6 hours (1 agent)

---

#### M3.3: Analyze Pipeline & Output (Days 20-23)

**What gets built:**
1. `jdlint_cli/src/analyze_pipeline.rs`
   - High-level orchestration: load config → discover files → run analyze → format output
   - Parallel or sequential (configurable via --parallel flag)
   - Output formatting: JSON (structured), text (human-readable), quiet (exit code only)
2. Tests: `jdlint_cli/tests/pipeline_tests.rs`
   - Mock rules; verify diagnostics flow through pipeline
   - Test JSON and text output formats
   - Verify exit codes (0 if no errors, 1+ if errors found)

**Tests written:**
- Unit: output formatting, exit code logic
- Integration: end-to-end check on 10-file jfit sample; verify diagnostics
- Snapshot: JSON and text output formats

**Exit criteria:**
- `jdlint check .` works on jfit mobile lib
- Output is parseable (JSON) and human-readable (text)
- Exit code matches error count
- Parallel execution optional but available

**Estimated effort:** 12-16 hours (1-2 agents)

---

#### M3.4: CLI Checkpoint & Review (End of Week 5)

**Activities:**
- Manual test: `jdlint check` on jfit mobile lib (no rules yet, just parser + infrastructure)
- Performance profile: measure file discovery + parse + empty analyze
- Code review: CLI design, output format, error handling

**Exit criteria (gating M4):**
- CLI fully functional for empty rule set
- Parser runs cleanly on jfit corpus
- Performance <500ms for full jfit lib (parser + infrastructure, no rules)

---

### M4: Rule Porting & Implementation (M4.0-M4.8) (Weeks 6-7, Days 24-37) — Complexity: **LARGE**

This phase is the largest, with 60 rules to port. Parallelization is critical. M0.5 (Trait Contracts, Rule Analysis Matrix) gates this phase.

#### M4.0: Rule Specification Audit & Golden Test Corpus (Days 24-25)

**What gets built (before rule implementation):**
1. `jdlint_rules/docs/rule_specs.md` — detailed spec for each of 60 rules
   - Rule name, category (error prevention, code style, maintainability, Flutter-specific)
   - Enabled by default in jfit analysis_options.yaml? (Yes/No)
   - Configuration options (e.g., no-magic-number: excluded_values, max-value)
   - Correct behavior on 5-10 code examples (reverse-engineered from dart_code_linter/pyramid_lint)
   - Edge cases and exceptions
2. Golden test corpus: `jdlint_rules/tests/corpus/{rule_name}/`
   - For each rule: 5-10 .dart files with expected diagnostics
   - Format: /* expect: rule_name, line 5, col 3 */ annotations in test files
   - All examples extracted from jfit or public Dart projects

**Activities:**
- Audit dart_code_linter source for each rule (1-2 hours per rule, 60 hours total)
- Audit pyramid_lint source (1 hour per rule, 26 hours total)
- Extract test cases from real jfit code and open-source examples
- Document rule behavior in specs
- Define "Rule Acceptance Criteria Policy": Same rule fires on same code (column-accurate), same message text (within 10% fuzzy match), same severity. Different suggestions/fixes acceptable in Phase 1.

**Tests written:**
- Specs: one per rule in rule_specs.md with acceptance criteria
- Golden corpus: 300-600 .dart test files (5-10 per rule)

**Exit criteria:**
- All 60 rules documented in rule_specs.md with behavior examples
- Golden corpus complete with expected annotations (/* expect: rule_name, line X, col Y */ format)
- Rule Acceptance Criteria Policy documented and approved
- Specs reviewed by team for accuracy and completeness
- Complexity tiers assigned (from M0.5 RULE_ANALYSIS_MATRIX) for resource allocation

**Estimated effort:** 80-100 hours (3-4 agents in parallel, 48-60 hours per agent)

---

#### M4.0.5: Golden Test Comparison Harness

**Threshold Empirical Justification (M4.0.5 Pre-Step):**
1. Before implementing the full harness, run a pilot on 10 representative rules:
   - 2 SIMPLE rules (e.g., avoid_dynamic, no_empty_block)
   - 5 MEDIUM rules (e.g., prefer_trailing_comma, newline_before_return, no_magic_number)
   - 3 COMPLEX rules (e.g., member_ordering, format_comment, no_equal_arguments)
2. Measure actual message text variance vs. dart_code_linter output on same code
3. Calibrate threshold:
   - If >95% exact match: raise threshold to 0.95
   - If 85-95% exact match: keep 0.85 default
   - If <80% exact match: investigate (spec mismatch? parser issue?) before proceeding
4. Document final threshold and rationale in `.omc/docs/RULE_ACCEPTANCE_CRITERIA.md`
5. Architect sign-off on threshold before M4.2 begins (add to M4.1 acceptance criteria)

**Acceptance:**
- Pilot run documented in RULE_ACCEPTANCE_CRITERIA.md
- Threshold justified empirically (not arbitrarily)
- Architect-approved threshold locked before M4.2

**What gets built (Main Harness):**
- File: `xtask/validate_rule_semantics/src/main.rs` (xtask binary)
- Functionality:
  - Load golden corpus annotations (`/* expect: rule_name, line X, col Y */`) from test files
  - Invoke jdlint on each corpus file; parse diagnostic output
  - Compare diagnostic positions:
    * Line: exact match required
    * Column: within 1 token boundary (≤3 byte offset for ASCII context)
    * Message: fuzzy match using `strsim::jaro_winkler >= 0.85` (tunable via `--threshold` flag)
    * Severity: exact match required
  - Generate report: pass/fail per rule per file, with delta score for fuzzy matches
  - Exit code 1 if any fuzzy match score < threshold; exit 0 if all pass
  - Output formats: JSON (for CI) and text (for humans)
- Dependencies: `strsim` crate for fuzzy string matching; `serde_json` for output
- Usage: `cargo xtask validate-rules --corpus jdlint_rules/tests/corpus/ --threshold 0.85`
- M4.0 deliverable set (updated):
  1. `jdlint_rules/docs/rule_specs.md` — detailed specs for all 60 rules (existing)
  2. Golden corpus .dart files with `/* expect: */` annotations (existing)
  3. `xtask/validate_rule_semantics/` — comparison harness binary (NEW)
  4. Architect sign-off on fuzzy match threshold (default 0.85) (NEW)
  5. `RULE_ACCEPTANCE_CRITERIA.md` — empirically-justified threshold documentation (NEW)
- Acceptance:
  - Harness builds and runs on at least 5 pilot rules before M4.2 begins
  - CI job added: `cargo xtask validate-rules` runs on every PR touching jdlint_rules
  - Zero tolerance: all rules must score >= 0.85 on golden corpus before PR can merge
- Estimated effort: 6-8 hours (1 agent)

**M4.0 Updated Effort:** 86-108 hours (adding M4.0.5: +8h)

---

#### M4.0.6: Spec Incompleteness Contingency

**If a rule's correct behavior remains ambiguous after 4 hours of reverse-engineering 
(source code + test output):**
1. Escalate to Architect async (non-blocking to other rules)
2. Implement rule with best-effort interpretation; annotate with `// SPEC: TBD - behavior chosen: <rationale>`
3. Golden corpus test documents the chosen behavior (test becomes the spec)
4. Spec updated post-implementation; TBD annotation removed
5. Maximum 5 rules may be in TBD state simultaneously; if exceeded, halt M4.2 and resolve

---

#### M4.1: Implementation Framework & Macros (Days 25-27)

**What gets built:**
1. `jdlint_analyze_macros/src/lib.rs` (if needed) — derive macros for rule boilerplate
   - Optional: simplify rule visitor impl
2. `xtask/codegen/rule_template.rs` — generate rule stub
   - Invoked as: `cargo xtask codegen rule --name avoid_dynamic`
   - Generates: `jdlint_rules/src/rules/avoid_dynamic.rs` with trait impl stub
3. Rule registry: `jdlint_rules/src/lib.rs`
   - Macro or function to register all rules
   - Example: `register_rules!(avoid_dynamic, no_magic_number, ...)`

**Tests written:**
- Unit: codegen template produces valid Rust
- Integration: generated stub compiles, implements Rule trait

**Exit criteria:**
- Codegen tool creates valid rule stubs
- Rule registry compiles and can instantiate all rules
- Framework is ready for M4.2-M4.6

**Estimated effort:** 8-10 hours (1 agent)

---

#### M4.2-M4.6: Rule Implementation (Days 27-37) — 6 phases, 10 rules per phase

**Organization:**
- Phase M4.2: `avoid_dynamic`, `avoid_passing_async_when_sync_expected`, `avoid_redundant_async`, `avoid_throw_in_catch_block`, `avoid_unnecessary_type_assertions`, `avoid_unnecessary_type_casts`, `avoid_unrelated_type_assertions`, `avoid_unused_parameters`, `avoid_nested_conditional_expressions`, `avoid_non_null_assertion`
- Phase M4.3: `avoid_late_keyword`, `avoid_global_state`, `prefer_async_await`, `prefer_correct_identifier_length`, `prefer_correct_type_name`, `prefer_conditional_expressions`, `prefer_first`, `prefer_immediate_return`, `prefer_iterable_of`, `prefer_last`
- Phase M4.4: `prefer_moving_to_variable`, `prefer_trailing_comma`, `double_literal_format`, `format_comment`, `member_ordering`, `newline_before_return`, `no_boolean_literal_compare`, `no_empty_block`, `no_equal_arguments`, `no_equal_then_else`
- Phase M4.5: `no_magic_number`, `no_object_declaration`, `use_design_system_item` (dart_code_linter), + 10 pyramid_lint rules
- Phase M4.6: Remaining pyramid_lint rules (proper_controller_dispose, proper_edge_insets_constructor, etc.)

**Per rule:**
1. Implement `Rule::analyze()` method
2. Visitor logic to detect violations
3. Diagnostic emission with correct span and message
4. Configuration handling (if rule has options, e.g., no_magic_number thresholds)
5. Unit tests: 5-10 test cases per rule (valid code, violations, edge cases)
6. Golden corpus tests: run rule on all jfit examples; verify matches expected diagnostics
7. Code review with rule spec author (if available)

**Tests written (per rule):**
- Unit: 5-10 cases covering violations and valid code
- Golden: 5-10 jfit code examples; compare against expected annotations
- Integration: rule runs in full jdlint pipeline; diagnostics collected correctly
- Snapshot: diagnostic output for each test case

**Exit criteria (per rule):**
- Diagnostic output matches golden corpus
- Rule configuration (if any) works correctly
- Unit tests all pass
- Code review approved

**Estimated effort:**
- Implementation: 2-3 hours per rule (120-180 hours total)
- Testing: 1-2 hours per rule (60-120 hours total)
- Code review: 0.5 hours per rule (30 hours total)
- **Total M4.2-M4.6: 210-330 hours** (6 agents in parallel, 35-55 hours per agent over 2 weeks)

**Parallelization:**
- Assign 10 rules to each of 6 agents
- Each agent owns end-to-end implementation, testing, code review for their rules
- Integrate weekly into main crate
- Shared resources: rule spec doc, golden corpus, codegen framework

---

#### M4.7: Rule Integration & Cross-Rule Tests (Days 37-38)

**What gets built:**
1. All 60 rules compiled into single `jdlint_rules` crate
2. Rule registry tested with all 60 rules instantiated
3. Cross-rule interaction tests (optional): ensure rules don't interfere

**Tests written:**
- Integration: all rules run on 20-file jfit sample
- Performance: benchmark each rule (track cumulative overhead)
- Regression: golden corpus for all 60 rules

**Exit criteria:**
- All 60 rules compile together
- No name collisions or conflicts
- Golden corpus tests pass for all rules
- Performance <800ms for jfit mobile lib (rules + parser + infrastructure)

**Estimated effort:** 8-12 hours (shared by rule engineers)

---

#### M4.8: Rule Checkpoint & Performance Review (End of Week 7)

**Activities:**
- Code review: rule implementations, consistency across rules
- Performance profiling: identify slow rules; optimize if needed
- Corpus validation: run all 60 rules on full jfit lib; verify diagnostics match expectations

**Exit criteria (gating M5):**
- All rule unit tests pass
- Golden corpus tests pass for all rules
- Performance <1s on jfit mobile lib (or identified for optimization in M6)
- Rules ready for LSP integration

---

### M5: LSP Server Implementation (Weeks 7-9, Days 39-54) — Complexity: **LARGE**

#### M5.0: LSP Caching & Incremental Analysis Design (Days 39-40)

**Purpose:** Design caching and incremental analysis strategy before implementation. LSP architecture is critical and affects the Rule trait API (idempotency, state management).

**What gets built:**
1. `.omc/docs/LSP_CACHING_DESIGN.md` — Cache invalidation strategy and incremental analysis model
   - `LspState` struct: cached AST per file, diagnostics per file, config state
   - Cache invalidation triggers: file content change (onChange), config file change (config reload)
   - Incremental analysis: when file X changes, only re-parse/analyze file X; other files reuse cached AST
   - File watcher integration: monitor jdlint.json for changes; reload rules on change
   - Debouncing strategy: batch rapid onChange events (e.g., wait 500ms after last change before re-analyze)
   - Thread safety: LSP server is single-threaded per spec; no concurrent file analysis in LSP mode (sequential per file)
2. Review gate: Architect approval required before M5.1 implementation

**Tests written:**
- Design review (no code yet)
- Validation: cache invalidation logic is clear; no subtle bugs (e.g., stale AST with new config)

**Exit criteria:**
- LSP caching strategy is documented and approved
- Incremental analysis model is clear for M5.2 implementation
- Architect confirms design avoids data races

**Estimated effort:** 4-6 hours (1 agent + Architect review)

---

#### M5.1: LSP Foundation & Protocol Handling (Days 40-43)

**What gets built:**
1. `jdlint_lsp/src/lib.rs` — LSP server skeleton
   - Use `lsp-server` or `tower-lsp` crate for protocol handling
   - Implement `Server` trait / `LanguageServer` trait
   - Handle initialize, initialized, shutdown requests
   - Document parameter types and responses
2. Protocol handlers:
   - `onOpen`: file opened in editor; run analyze, publish diagnostics
   - `onChange`: file modified; re-analyze (debounce after 500ms)
   - `onSave`: file saved; re-analyze (with potential auto-fix in Phase 2)
   - `hover`: return type information (if available from parser)
   - `publishDiagnostics`: emit diagnostics to editor

3. Tests: `jdlint_lsp/tests/lsp_tests.rs`
   - Mock client; send initialize request; verify response
   - Mock file open/change/save; verify diagnostics published

**Tests written:**
- Unit: LSP message handling, request/response format, JSON-RPC 2.0 compliance
- Integration: mock LSP client sends sequence of requests; verify responses
- Snapshot: LSP protocol messages (JSON)
- Compliance: validate JSON-RPC 2.0 format and LSP 3.17 message types using `lsp-types` crate

**Exit criteria:**
- LSP server compiles and starts
- Initialize/initialized/shutdown cycle works
- Diagnostics publishing tested
- JSON-RPC 2.0 request/response format verified against spec
- LSP 3.17 message types (InitializeResult, PublishDiagnosticsParams, etc.) validated using `lsp-types` crate
- Unit tests validate message shapes by deserializing with `serde_json::from_str` against `lsp_types::` types

**Estimated effort:** 18-22 hours (1-2 agents)

---

#### M5.2: Incremental Analysis & Caching (Days 43-46)

**What gets built:**
1. `jdlint_lsp/src/state.rs` — LSP server state management
   - Cache: parsed AST per file (invalidated on change)
   - Diagnostics cache: prev diagnostics (for incremental updates)
   - Config reload on jdlint.json change
2. Incremental analysis: re-parse only changed file; reuse other files
3. Tests: `jdlint_lsp/tests/incremental_tests.rs`
   - Open file, modify, verify only that file re-analyzed
   - Open multiple files; verify independence
   - Config reload: modify jdlint.json, verify rules reloaded

**Tests written:**
- Unit: cache invalidation, config reload
- Integration: multi-file incremental analysis; verify no cascades

**Exit criteria:**
- Single-file change doesn't re-analyze entire workspace
- Config reload works
- Performance <100ms for single file re-analyze

**Estimated effort:** 12-16 hours (1 agent)

---

#### M5.3: VS Code Integration & Testing (Days 46-50)

**What gets built:**
1. VS Code extension (minimal): `extensions/jdlint-vscode/`
   - Extension manifest (package.json)
   - Client code: launch LSP server, show diagnostics
   - Configuration: path to jdlint binary
2. End-to-end test: open jfit .dart file in VS Code; verify diagnostics appear
3. Tests: Manual test in VS Code (cannot fully automate)

**Tests written:**
- Manual: open jfit code in VS Code; verify diagnostic rendering
- Integration: mock VS Code client; send LSP messages; verify diagnostics

**Exit criteria:**
- VS Code extension installs and runs
- jdlint binary launches from VS Code
- Diagnostics appear in editor
- No crashes or hangs

**Estimated effort:** 12-16 hours (1 agent)

---

#### M5.4: LSP Checkpoint & Review (End of Week 8)

**Activities:**
- End-to-end test: VS Code running jdlint LSP on jfit code
- Performance validation: incremental re-analyze is fast (<100ms)
- Code review: LSP server design, protocol compliance

**Exit criteria (gating M6):**
- LSP server fully functional
- VS Code integration works
- Performance acceptable
- Protocol compliance verified against LSP spec

---

### M6: Performance Optimization & Benchmarking (Weeks 9-10, Days 51-60) — Complexity: **MEDIUM**

#### M6.1: Benchmark Harness & Profiling Setup (Days 51-53)

**What gets built:**
1. `benches/jfit_mobile_bench.rs` — Criterion benchmarks
   - Full jfit mobile lib: measure parse time, analyze time, total time
   - Per-stage: parser alone, parser+analyze, parser+analyze+output
   - Variants: single-threaded vs. Rayon parallel
   - Reference hardware: Linux x86_64, modern CPU (8+ cores)
2. Profiling tools:
   - perf integration: `cargo bench` collects perf data
   - Flamegraph generation: `cargo flamegraph` on jfit corpus
3. Tests: verify benchmarks compile and run

**Tests written:**
- Integration: benchmark runs without error on jfit corpus

**Exit criteria:**
- Benchmarks establish baseline: current performance
- Target: <1000ms total (parser + analyze + output)
- Profiling data collected

**Estimated effort:** 8-10 hours (1 agent)

---

#### M6.2: Optimization Sprints (Days 53-57)

**If <1000ms achieved in M6.1:**
- Verify target met; proceed to M7
- Document optimization techniques used (string interning, AST compression, etc.)

**If >1000ms:**
1. Identify bottleneck: parser, rules, or I/O?
2. Apply targeted optimizations:
   - Parser: intern strings, cache regex compiles, reuse buffers
   - Rules: parallelize rule execution (per-rule Rayon scopes), avoid deep clones
   - I/O: batch file reads, async file writes (if applicable)
3. Re-benchmark after each optimization
4. Flamegraph-guided: focus on hot loops (>5% CPU time)

**Tests written:**
- Regression: benchmark before/after each optimization
- Unit: optimizations don't change diagnostic output

**Exit criteria:**
- jfit mobile lib analyzed in <1000ms
- No regression in rule diagnostics
- Optimizations documented in code comments

**Estimated effort:** 16-24 hours (1-2 agents, if optimization needed)

---

#### M6.3: Performance Checkpoint & Lock (End of Week 9)

**Activities:**
- Final benchmark run on reference hardware
- Document performance targets and methodology
- Lock performance: future changes must maintain <1000ms or justify regression

**Exit criteria (gating M7):**
- jfit mobile lib analyzed in <1000ms consistently
- Benchmark data committed to repo
- Performance methodology documented

---

### M7: Nix Flake & Integration with jfit (Week 10-11, Days 57-64) — Complexity: **MEDIUM**

#### M7.0: Nix Integration Planning & Early Validation (Week 1, alongside M0) — Complexity: SMALL

**Problem:** Nix integration is a hard acceptance criterion (spec: "jfit/flake.nix updated; jdlint available in devShell"). If M7.2 fails in week 10-11, minimal recovery time remains.

**What gets built:**
1. Assign Nix engineer (dedicated, named in M10 team table) from week 1 — not just "review in M7"
2. In M0.5 (week 1): Nix engineer reviews both jdlint/flake.nix design and jfit/flake.nix integration plan (2 hours)
   - Validates: input syntax, devShell overlay approach, buildRustPackage pattern
   - Documents risks in `.omc/docs/NIX_INTEGRATION_PLAN.md`
3. Early validation gates:
   - M7.0 (week 1): `nix flake check` on draft jdlint/flake.nix skeleton — verifies syntax
   - M7.1 (week 9): `nix build .#jdlint` — verifies derivation produces binary
   - M7.2 (week 8, parallel to M5): Test jdlint input in jfit flake — moved earlier

**Timing Change:**
- Move M7.2 (jfit integration test) from week 10-11 → week 8 (parallel to M5 LSP work)
- Nix engineer works on M7.2 while LSP engineers work on M5; no dependency between them
- This provides 3-4 weeks of recovery time if integration issues are found

**Acceptance:**
- NIX_INTEGRATION_PLAN.md committed in M0.5
- Nix engineer named in team structure
- M7.2 milestone now targets week 8

Also update the team structure section to indicate the Nix engineer is assigned from week 1.

---

#### M7.0.1: Nix Versioning Strategy Decision

**Decision (Locked):**
- **Phase 1 uses `path:` URL** for local development iteration: `jdlint.url = "path:/home/jacob/Documents/Developer/jdlint"`
  - Rationale: Rapid iteration; allows jfit to pick up jdlint changes immediately (no tag/release cycle needed during Phase 1)
  - Non-blocking: path: URL works for local flake.nix evaluation
- **Phase 2 will switch to `github:` URL with hash-pinning**: `jdlint.url = "github:jacobsanderson/jdlint/<tag>"`
  - Rationale: Reproducibility in jfit CI; explicit version pinning; production deployment

**Documentation:** Recorded in open-questions.md decision log.

---

#### M7.1: jdlint Flake & Build Derivation (Days 57-58)

**What gets built:**
1. `flake.nix` at jdlint repo root (already sketched in spec)
   - Build derivation: `buildRustPackage` for jdlint binary
   - Inputs: nixpkgs, flake-utils, Rust stable
   - Outputs: jdlint binary in default package
2. Tests:
   - `nix build .#jdlint` produces binary at result/bin/jdlint
   - Binary runs: `result/bin/jdlint --version` outputs semver
   - `nix flake check` validates flake.nix

**Tests written:**
- Integration: nix build produces executable
- Smoke test: binary is runnable and has correct permissions

**Exit criteria:**
- `nix build .` produces jdlint binary
- Binary runs and has expected behavior
- flake.nix is valid Nix

**Estimated effort:** 4-6 hours (1 agent)

---

#### M7.2: jfit Integration (Days 58-61)

**What gets built:**
1. Update `/home/jacob/Documents/Developer/jfit/flake.nix`:
   - Add input: `jdlint.url = "path:/home/jacob/Documents/Developer/jdlint"` (Phase 1 strategy per M7.0)
   - Add package: `jdlint = jdlint.packages.${system}.default`
   - Add to devShell: `pkgs.jdlint`, `jdlint.packages.${system}.default`
   - Documentation: note that Phase 2 will switch to `github:` URL with hash-pinning
2. Tests:
   - `nix develop` in jfit; verify jdlint on PATH
   - `jdlint check apps/mobile/lib` runs in jfit devShell

**Tests written:**
- Integration: jfit flake.nix evaluates with jdlint input (path: URL)
- Smoke test: jdlint available in devShell

**Exit criteria:**
- jfit flake accepts jdlint input (path: URL)
- jdlint binary available in jfit devShell
- `nix develop` in jfit makes jdlint available
- versioning strategy documented for Phase 2 transition

**Estimated effort:** 6-8 hours (1 agent)

---

#### M7.3: Nix Checkpoint & Integration Validation (End of Week 11)

**Activities:**
- Manual test: `nix develop` in jfit; run `jdlint check` on jfit mobile lib
- Verify jdlint runs, emits diagnostics, exits correctly
- Document jfit integration in README

**Exit criteria (gating M8):**
- jdlint fully integrated into jfit flake
- jdlint available in jfit devShell
- CI pipeline includes jdlint check

---

### M8: Full Integration Testing & Corpus Validation (Week 11, Days 64-70) — Complexity: **LARGE**

#### M8.1: Corpus Regression Tests (Days 64-66)

**What gets built:**
1. `tests/corpus_regression.rs` — comprehensive corpus test
   - Run all 60 rules on full jfit mobile lib (214 files)
   - Compare diagnostics against golden expectations
   - Generate report: pass/fail per rule per file
2. Golden diagnostics snapshot: `tests/snapshots/jfit_mobile_corpus.json`
   - JSON format: file, rule, line, col, message, severity
   - Commit to git; review on any change

**Tests written:**
- Integration: full corpus analysis with diagnostics validation
- Snapshot: diagnostic output for all files and rules

**Exit criteria:**
- 100% of expected diagnostics found
- Zero false positives
- Snapshot committed and reviewable

**Estimated effort:** 12-16 hours (1-2 agents)

---

#### M8.2: CLI Acceptance Tests (Days 66-68)

**What gets built:**
1. CLI test suite: `jdlint_cli/tests/acceptance_tests.rs`
   - `jdlint check .` on jfit mobile lib
   - Verify exit code (1 if diagnostics found)
   - Verify JSON output format
   - Verify text output format
   - Test exclude patterns
   - Test config override flags
2. Performance test: measure `jdlint check` on jfit (must be <1s)

**Tests written:**
- Integration: end-to-end CLI invocations
- Performance: benchmark CLI run
- Snapshot: CLI output formats

**Exit criteria:**
- CLI acceptance tests pass
- Performance <1s on jfit mobile lib
- Output formats are correct and parseable

**Estimated effort:** 8-12 hours (1 agent)

---

#### M8.3: LSP Acceptance Tests (Days 68-70)

**What gets built:**
1. LSP test suite: `jdlint_lsp/tests/acceptance_tests.rs`
   - Mock LSP client; simulate VS Code workflow:
     1. Initialize
     2. Open jfit .dart file
     3. Verify diagnostics published
     4. Modify file
     5. Verify re-analysis
     6. Save file
     7. Verify final diagnostics
   - Test config reload
   - Test multiple concurrent files
2. Performance test: incremental re-analyze must be <100ms per file

**Tests written:**
- Integration: full LSP workflow tests
- Performance: incremental re-analyze timing
- Snapshot: LSP protocol messages

**Exit criteria:**
- LSP acceptance tests pass
- Incremental re-analyze is fast (<100ms)
- No crashes or hangs in LSP server

**Estimated effort:** 12-16 hours (1-2 agents)

---

#### M8.4: Full Integration Checkpoint (End of Week 10)

**Activities:**
- Run all acceptance tests together
- Verify no conflicts between CLI and LSP modes
- Performance validation on jfit corpus
- Code review of integration tests

**Exit criteria (gating M9):**
- All corpus, CLI, LSP acceptance tests pass
- Performance <1s on jfit mobile lib
- Zero known bugs in Phase 1 feature set
- System is production-ready for Phase 1 release

---

### M9: Documentation, Cleanup, & Phase 1 Release (Week 11-12, Days 70-77) — Complexity: **MEDIUM**

#### M9.1: README & User Documentation (Days 70-71)

**What gets built:**
1. `README.md` — jdlint overview, installation, usage
   - What is jdlint? (Rust-based Dart linter)
   - Installation: `nix develop` in jfit or standalone `cargo build`
   - Usage: `jdlint check .`, `jdlint --config jdlint.json`, LSP setup
   - Configuration: jdlint.json schema with examples
   - Troubleshooting
2. `ARCHITECTURE.md` — design overview
   - Crate structure and responsibilities
   - Parser design and grammar
   - Rule trait and visitor pattern
   - LSP server architecture
3. `CONTRIBUTING.md` — for future contributors
   - How to add new rules (M4 pattern)
   - Testing expectations
   - Code review guidelines

**VS Code Extension Distribution (Phase 1):**
- Source location: `extensions/jdlint-vscode/` in jdlint repo
- Phase 1 installation method:
  ```
  # Option A: Install from jdlint repo (manual)
  git clone https://github.com/jacobdev/jdlint
  cd jdlint
  code --install-extension ./extensions/jdlint-vscode/
  
  # Option B: Install from jfit devShell (automatic if bundled)
  nix develop  # jfit devShell
  # jdlint-vscode bundled if configured in jfit devShell
  ```
- README.md installation instructions include both options
- Phase 2 target: Publish to VS Code Marketplace (auto-updates)
- Open question: Whether bundling extension in jfit devShell is in scope for Phase 1 (defer to M9 decision)

**Tests written:**
- Documentation examples are correct (code snippets compile/run)

**Exit criteria:**
- README covers installation, usage, configuration
- ARCHITECTURE documents design decisions
- CONTRIBUTING guides future work
- VS Code extension installation instructions documented

**Estimated effort:** 6-8 hours (1 agent)

---

#### M9.2: Code Cleanup & Quality Review (Days 71-73)

**What gets built:**
1. Clippy pass: `cargo clippy --workspace --all-targets`
2. Format pass: `cargo fmt --all`
3. Test coverage review: ensure all crates have >80% line coverage
4. Dead code removal
5. Documentation coverage: all public items have doc comments

**Tests written:**
- CI linting: clippy, fmt, coverage checks

**Exit criteria:**
- Zero clippy warnings
- All code formatted
- >80% test coverage per crate
- All public APIs documented

**Estimated effort:** 8-12 hours (1-2 agents)

---

#### M9.3: CI/CD & Release Preparation (Days 73-75)

**What gets built:**
1. GitHub Actions workflow: `.github/workflows/ci.yml`
   - Lint (clippy, fmt)
   - Test (cargo test --all)
   - Benchmark (cargo bench)
   - Coverage (codecov or tarpaulin)
   - Nix build: `nix build .`
   - Release: on tag, build and publish binary (optional for Phase 1)
2. Release checklist: README.md section
3. Versioning: semver in Cargo.toml

**Tests written:**
- CI pipeline runs end-to-end

**Exit criteria:**
- CI pipeline is automated
- All checks pass on main branch
- Release process is documented

**Estimated effort:** 6-8 hours (1 agent)

---

#### M9.4: Phase 1 Release & Handoff (Days 75-77)

**Activities:**
1. Final integration test run (all M8 tests)
2. Performance validation (jfit mobile lib <1s)
3. Documentation review
4. Tag release: `v0.1.0` or similar
5. Publish to GitHub releases (optional for Phase 1, but recommended)
6. Update jfit flake to reference release version (instead of path)
7. Final code review by architect

**Exit criteria (Phase 1 Complete):**
- All M0-M9 tests pass
- jfit mobile lib analyzed in <1s
- All 60 rules ported and working
- LSP server functional in VS Code
- Nix flake integration verified
- Documentation complete
- CI/CD pipeline automated
- Ready for Phase 2 or production deployment

**Estimated effort:** 4-6 hours (1 agent)

---

## 4. Detailed Implementation Steps (Per Milestone)

### M0: Project Setup & Infrastructure

**Step M0.1: Rename repo from jlint to jdlint**
- Crate: core workspace
- Acceptance: `Cargo.toml` has `name = "jdlint"`, all refs updated

**Step M0.2: Scaffold workspace Cargo.toml**
- File: `Cargo.toml`
- Content: `[workspace]` with members: jdlint_dart_parser, jdlint_syntax, jdlint_analyze, jdlint_rules, jdlint_diagnostics, jdlint_lsp, jdlint_config, jdlint_cli, jdlint (binary)
- Acceptance: `cargo build --workspace` compiles all 9 crates

**Step M0.3: Create flake.nix**
- File: `flake.nix`
- Content: Rust stable, build derivation, devShell with rustc, cargo, clippy, fmt
- Acceptance: `nix flake check` passes, `nix build .` works

**Step M0.4: Setup GitHub Actions CI**
- File: `.github/workflows/ci.yml`
- Jobs: lint, test, bench, coverage
- Acceptance: CI triggers on push; all checks pass

**Step M0.5: Create planning documents**
- File: `.omc/plans/jdlint-rules-matrix.md`
- Content: Mapping of 60 rules to crates/agents
- Acceptance: Matrix is complete and assignable

---

### M0.5: Trait Contracts & Grammar Scope Definition

**Step M0.5.1: Trait Contracts Document**
- File: `.omc/docs/TRAIT_CONTRACTS.md`
- Content: Rust trait specifications for Rule, Visitor, AnalyzeContext (see M0.5 milestone section above)
- Example: end-to-end avoid_dynamic rule implementation
- Acceptance: Architect approves trait design

**Step M0.5.2: Parser Grammar Scope Document**
- File: `.omc/docs/PARSER_GRAMMAR.md`
- Content: Dart 3.x grammar scope for jdlint Phase 1 (see M0.5 milestone section above)
- Acceptance: Scope is clear; no ambiguities about what to parse

**Step M0.5.3: Rule Analysis Matrix**
- File: `.omc/docs/RULE_ANALYSIS_MATRIX.md`
- Content: Table of 60 rules with complexity tier, semantic tags (see M0.5 milestone section above)
- Acceptance: All rules categorized; resource allocation accurate

**Step M0.5.4: Parallelism Model Document**
- File: `.omc/docs/PARALLELISM_MODEL.md`
- Content: Per-file vs. per-rule decision; baseline measurement plan (see M0.5 milestone section above)
- Acceptance: Model is clear for M2 and M6 implementation

**Step M0.5.5: M0.5 Gate**
- Activity: Architect review of all 4 documents; sign-off
- Acceptance: All documents approved; M1/M2/M4 teams can proceed with confidence

---

### M1.1: Grammar & Lexer

**Step M1.1.1: Create jdlint_dart_parser crate**
- Crate: jdlint_dart_parser
- File: `crates/jdlint_dart_parser/Cargo.toml`
- Acceptance: crate compiles

**Step M1.1.2: Implement Lexer**
- Crate: jdlint_dart_parser
- File: `crates/jdlint_dart_parser/src/lexer.rs`
- Types: `Token`, `TokenKind` (IDENTIFIER, NUMBER, STRING, KEYWORD, OPERATOR, PUNCT, WHITESPACE, COMMENT, ERROR)
- Methods: `Lexer::new(source: &str)`, `next_token() -> Token`
- Features: multiline strings, string interpolation, comments
- Acceptance: lexer handles all Dart token types; no panics

**Step M1.1.3: Test lexer**
- File: `crates/jdlint_dart_parser/tests/lexer_tests.rs`
- Tests: 50 test cases (keywords, operators, strings, comments)
- Snapshot: token stream (insta)
- Acceptance: all 50 tests pass; snapshots committed

**Step M1.1.4: Grammar reference**
- File: `crates/jdlint_dart_parser/src/grammar.md`
- Content: Full Dart 3.x grammar (from spec), with Phase 1 rules marked
- Acceptance: document is complete and used by M1.2 parser

---

### M1.2: Recursive Descent Parser Core

**Step M1.2.1: Parser skeleton**
- Crate: jdlint_dart_parser
- File: `crates/jdlint_dart_parser/src/parser.rs`
- Struct: `Parser { lexer: Lexer, current_token: Token }`
- Methods: `new()`, `parse_compilation_unit()`, `peek()`, `advance()`, error recovery
- Acceptance: parser compiles; basic structure in place

**Step M1.2.2: Expression parsing (precedence climbing)**
- File: `crates/jdlint_dart_parser/src/parser.rs` (extended)
- Methods: `parse_expression()`, `parse_assignment()`, `parse_ternary()`, `parse_logical_or()`, etc.
- Operator precedence table (document in comments)
- Acceptance: expression parsing tests pass

**Step M1.2.3: Statement & declaration parsing**
- File: `crates/jdlint_dart_parser/src/parser.rs` (extended)
- Methods: `parse_statement()`, `parse_class_declaration()`, `parse_function_declaration()`, `parse_if_statement()`, `parse_block()`, etc.
- Error recovery: skip to statement boundary on error
- Acceptance: statement parsing tests pass; error recovery tested

**Step M1.2.4: Type & pattern parsing**
- File: `crates/jdlint_dart_parser/src/parser.rs` (extended)
- Methods: `parse_type()`, `parse_type_parameters()`, `parse_pattern()` (Dart 3.x patterns, records, sealed classes)
- Acceptance: type and pattern tests pass

**Step M1.2.5: Parser tests**
- File: `crates/jdlint_dart_parser/tests/parser_tests.rs`
- Tests: 100+ cases covering all productions (valid, edge case, error recovery)
- Snapshot: AST structure (insta)
- Acceptance: all 100+ tests pass; snapshots committed

**Step M1.2.6: Parser integration on jfit corpus**
- Activity: Run parser on all 214 jfit .dart files
- Acceptance: no panics; parse trees match expected structure

---

### M1.3: AST & Syntax Crate

**Step M1.3.1: Create jdlint_syntax crate**
- Crate: jdlint_syntax
- File: `crates/jdlint_syntax/Cargo.toml`
- Acceptance: crate compiles

**Step M1.3.2: Define AST node types**
- Crate: jdlint_syntax
- File: `crates/jdlint_syntax/src/ast.rs`
- Types: `Program`, `ClassDecl`, `FunctionDecl`, `MethodDecl`, `ExprStmt`, `IfStmt`, `Block`, `Expression`, `Pattern`, `Type`, etc. (enum-based)
- Span tracking: each node has `Span { start: usize, end: usize }`
- Acceptance: all nodes compile; span tracking verified

**Step M1.3.3: SyntaxKind enum**
- File: `crates/jdlint_syntax/src/syntax_kind.rs`
- Enum: `SyntaxKind` with variant per node type
- Methods: classify nodes for pattern matching
- Acceptance: SyntaxKind complete; used by visitor (M1.3.4)

**Step M1.3.4: Visitor trait**
- File: `crates/jdlint_syntax/src/visitor.rs`
- Trait: `Visitor<R> { fn visit_program(&mut self, p: &Program) -> R, ... }`
- Default implementation: walks full tree
- Return type R: allows rules to accumulate diagnostics
- Acceptance: visitor trait is ergonomic; tested in analyze (M2.2)

**Step M1.3.5: Syntax tests**
- File: `crates/jdlint_syntax/tests/ast_tests.rs`
- Tests: AST node construction, span tracking, visitor pattern
- Acceptance: all tests pass

---

### M1.4: Parser Integration Tests

**Step M1.4.1: Corpus test harness**
- Crate: jdlint_dart_parser
- File: `crates/jdlint_dart_parser/tests/corpus_tests.rs`
- Activity: Iterate all 214 jfit .dart files; parse and snapshot
- Acceptance: zero panics; all files parse to completion

**Step M1.4.2: Snapshot commit**
- File: `crates/jdlint_dart_parser/tests/snapshots/` (insta generated)
- Activity: Commit snapshot per file; review for correctness
- Acceptance: snapshots reviewed by architect

---

### M2.1: Diagnostics Types & Serialization

**Step M2.1.1: Create jdlint_diagnostics crate**
- Crate: jdlint_diagnostics
- File: `crates/jdlint_diagnostics/Cargo.toml`
- Deps: serde, serde_json (for serialization)
- Acceptance: crate compiles

**Step M2.1.2: Define Diagnostic types**
- File: `crates/jdlint_diagnostics/src/lib.rs`
- Types: `Diagnostic { code: String, message: String, severity: Severity, span: Span, suggestions: Vec<Suggestion> }`, `Severity`, `Span`, `Suggestion`
- Methods: constructors, serialization (JSON, LSP, text)
- Acceptance: all types defined and serializable

**Step M2.1.3: Diagnostic tests**
- File: `crates/jdlint_diagnostics/tests/diag_tests.rs`
- Tests: construct diagnostics, serialize to JSON/LSP/text, verify formats
- Snapshot: diagnostic output formats (insta)
- Acceptance: all tests pass; formats are correct

---

### M2.2: Analyze & Rule Traits

**Step M2.2.1: Create jdlint_analyze crate**
- Crate: jdlint_analyze
- File: `crates/jdlint_analyze/Cargo.toml`
- Deps: jdlint_syntax, jdlint_diagnostics, rayon
- Acceptance: crate compiles

**Step M2.2.2: Define Rule trait**
- File: `crates/jdlint_analyze/src/rule.rs`
- Trait: `Rule { fn name(&self) -> &'static str, fn analyze(&self, tree: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> }`
- Acceptance: trait is ergonomic; used by rules (M4)

**Step M2.2.3: RuleVisitor trait**
- File: `crates/jdlint_analyze/src/visitor.rs`
- Trait: `RuleVisitor: Visitor<Vec<Diagnostic>>` (extends Visitor from jdlint_syntax)
- Methods: per-node visit methods; default walks tree
- Acceptance: trait is easy to implement for rules

**Step M2.2.4: AnalyzeContext**
- File: `crates/jdlint_analyze/src/context.rs`
- Struct: `AnalyzeContext { config: JdlintConfig, file_path: PathBuf, source: String, diags: Vec<Diagnostic> }`
- Methods: add_diagnostic(), get_config(), get_line_col()
- Acceptance: context provides all info rules need

**Step M2.2.5: RuleRegistry**
- File: `crates/jdlint_analyze/src/registry.rs`
- Struct: `RuleRegistry { rules: HashMap<String, Box<dyn Rule>> }`
- Methods: register_rule(), lookup_rule(), run_all()
- Acceptance: registry can hold all 60 rules; rules registered correctly

**Step M2.2.6: Parallel analysis**
- File: `crates/jdlint_analyze/src/parallel.rs`
- Function: `analyze_parallel(rules: &RuleRegistry, files: Vec<(Path, Source)>, config: &JdlintConfig) -> Vec<Diagnostic>`
- Uses Rayon work-stealing
- Thread safety: no shared mutable state; each thread has own diagnostics
- Acceptance: parallel analysis tested on 10-file sample; no data races

**Step M2.2.7: Analyze tests**
- File: `crates/jdlint_analyze/tests/analyze_tests.rs`
- Tests: rule trait, visitor pattern, context passing, parallel execution
- Acceptance: all tests pass; parallel execution verified safe

---

### M2.3: Config Deserialization

**Step M2.3.1: Create jdlint_config crate**
- Crate: jdlint_config
- File: `crates/jdlint_config/Cargo.toml`
- Deps: serde, serde_json
- Acceptance: crate compiles

**Step M2.3.2: Define JdlintConfig types**
- File: `crates/jdlint_config/src/lib.rs`
- Struct: `JdlintConfig { rules: HashMap<String, RuleConfig>, exclude_patterns: Vec<String>, severity_override: HashMap<String, Severity>, max_errors: Option<usize> }`
- Struct: `RuleConfig { enabled: bool, options: HashMap<String, serde_json::Value> }`
- Serde derives: #[derive(Serialize, Deserialize)]
- Acceptance: types compile and serialize correctly

**Step M2.3.3: Default config**
- File: `jdlint.json` (in repo root)
- Content: all 60 Phase 1 rules enabled; example options for configurable rules
- Acceptance: jdlint.json loads without error

**Step M2.3.4: Config loading**
- File: `crates/jdlint_config/src/loader.rs`
- Function: `load_config(path: &Path) -> Result<JdlintConfig, ConfigError>`
- Handles: missing file (use defaults), invalid JSON, missing fields (use defaults)
- Acceptance: can load jdlint.json and default

**Step M2.3.5: Config tests**
- File: `crates/jdlint_config/tests/config_tests.rs`
- Tests: load valid, invalid, partial configs; verify defaults applied
- Snapshot: config deserialization output
- Acceptance: all tests pass; defaults work correctly

---

### M2.4: Observability Infrastructure

**Step M2.4.1: Tracing crate setup**
- Crate: jdlint (or new jdlint_tracing if separate)
- File: Add `tracing` dependency to relevant crates
- Setup: Initialize tracing subscriber in CLI main (info level by default)
- Acceptance: tracing crate compiles; macros available

**Step M2.4.2: Logging infrastructure**
- Files: Update jdlint_dart_parser, jdlint_analyze, jdlint_lsp, jdlint_cli with logging
- Content: Add `debug!()`, `info!()` calls at major checkpoints (parse start/end, rule execution, LSP messages)
- Log levels: TRACE (rare), DEBUG (per-node, rule details), INFO (file start/end, summary), WARN (unexpected), ERROR (failures)
- Acceptance: logs are emitted at correct levels

**Step M2.4.3: CLI --verbose flag**
- File: `crates/jdlint_cli/src/args.rs`
- Content: Add --verbose flag; when set, initialize tracing with DEBUG level instead of INFO
- Acceptance: `jdlint check . --verbose` shows DEBUG logs

**Step M2.4.4: Observability tests**
- File: `crates/jdlint_cli/tests/logging_tests.rs`
- Tests: verify logs at correct levels, --verbose flag works, log format is structured
- Snapshot: log output for sample analysis
- Acceptance: all tests pass; log output is parseable

---

### M3.1: CLI Skeleton with Clap

**Step M3.1.1: Create jdlint_cli crate & binary crate**
- Crates: jdlint_cli, jdlint
- Files: `crates/jdlint_cli/Cargo.toml`, `crates/jdlint/Cargo.toml`
- Deps: clap (derive), jdlint_analyze, jdlint_config
- Acceptance: crates compile

**Step M3.1.2: CLI args & subcommands**
- File: `crates/jdlint_cli/src/args.rs`
- Struct: `Args` with derive(Parser) — see Clap documentation
- Subcommands: `check { path: PathBuf }`, `lsp { }`, `version { }`
- Flags: `--config <path>`, `--exclude <glob>`, `--format json|text`, `--quiet`, `--exit-code <n>`
- Acceptance: `jdlint --help`, `jdlint check --help` work correctly

**Step M3.1.3: Main entry point**
- File: `crates/jdlint/src/main.rs`
- Code: parse args, dispatch to subcommand, exit with code
- Acceptance: `jdlint --version` outputs semver; `jdlint check` with missing args shows help

**Step M3.1.4: CLI tests**
- File: `crates/jdlint_cli/tests/cli_tests.rs`
- Tests: arg parsing, help output, version flag
- Acceptance: all tests pass

---

### M3.2: File Discovery & Walk

**Step M3.2.1: File walker**
- File: `crates/jdlint_cli/src/file_walker.rs`
- Function: `walk_directory(root: &Path, exclude: &[String]) -> Result<Vec<(Path, SourceCode)>, Error>`
- Uses: walkdir or similar; filters by .dart extension; respects exclude patterns
- Handles: permissions errors gracefully (skip file with warning)
- Acceptance: can walk jfit mobile lib; finds all 214 files

**Step M3.2.2: File walker tests**
- File: `crates/jdlint_cli/tests/walker_tests.rs`
- Tests: walk synthetic directory tree; test exclude patterns (glob syntax)
- Integration: walk jfit mobile lib; verify count
- Acceptance: all tests pass; 214 files found in jfit

---

### M3.3: Analyze Pipeline & Output

**Step M3.3.1: Analyze pipeline orchestration**
- File: `crates/jdlint_cli/src/pipeline.rs`
- Function: `run_check(path: &Path, config: &JdlintConfig, parallel: bool) -> Result<CheckOutput, Error>`
- Steps: load config → discover files → run analyze (sequential or parallel) → format output
- Output struct: `CheckOutput { diagnostics: Vec<Diagnostic>, total_files: usize, exit_code: i32 }`
- Acceptance: pipeline works end-to-end

**Step M3.3.2: Output formatting**
- File: `crates/jdlint_cli/src/output.rs`
- Functions: `format_json(diags) -> String`, `format_text(diags) -> String`, `format_quiet(diags) -> String`
- Acceptance: formats are correct and parseable

**Step M3.3.3: Pipeline tests**
- File: `crates/jdlint_cli/tests/pipeline_tests.rs`
- Tests: end-to-end check on 10-file jfit sample; verify diagnostics flow; test output formats
- Snapshot: JSON and text output
- Acceptance: all tests pass; exit codes correct

---

### M4: Rule Implementation (Generic pattern, repeated 60 times)

For each rule (e.g., `avoid_dynamic`):

**Step M4.X.1: Create rule file**
- Crate: jdlint_rules
- File: `crates/jdlint_rules/src/rules/avoid_dynamic.rs`
- Struct: `AvoidDynamic;`
- Impl: `Rule for AvoidDynamic { fn name() -> &'static str { "avoid_dynamic" }, fn analyze(...) { ... } }`
- Impl: `RuleVisitor for AvoidDynamic` (if using visitor pattern)
- Acceptance: rule compiles; trait impl complete

**Step M4.X.2: Rule logic**
- File: `crates/jdlint_rules/src/rules/avoid_dynamic.rs` (extended)
- Logic: detect violations (e.g., `dynamic` keyword usage)
- Diagnostic emission: collect violations; emit diagnostic per violation
- Configuration: if rule has options, read from `AnalyzeContext::config`
- Acceptance: logic matches rule spec

**Step M4.X.3: Unit tests**
- File: `crates/jdlint_rules/src/rules/avoid_dynamic_tests.rs` or `tests/rules/avoid_dynamic_tests.rs`
- Tests: 5-10 cases (valid code, violations, edge cases)
- Snapshot: diagnostic output
- Acceptance: all tests pass; output matches spec

**Step M4.X.4: Golden corpus tests**
- File: `crates/jdlint_rules/tests/corpus/avoid_dynamic/`
- Files: 5-10 .dart files with `/* expect: ... */` annotations
- Activity: run rule on corpus files; compare against expected
- Acceptance: no mismatches; diagnostics match expected

---

### M5.0: LSP Caching & Incremental Analysis Design

**Step M5.0.1: LSP Caching Design Document**
- File: `.omc/docs/LSP_CACHING_DESIGN.md`
- Content: Cache invalidation triggers, LspState struct definition, incremental analysis model (see M5.0 milestone section above)
- Acceptance: Architect approves design; no subtle data race issues

**Step M5.0.2: Design Review**
- Activity: Architect review of LSP caching design
- Acceptance: Design avoids stale AST + new config bug; thread safety confirmed

---

### M5.1-M5.3: LSP Implementation (Covered above in milestone section)

Key files:
- `crates/jdlint_lsp/src/lib.rs` — LSP server
- `crates/jdlint_lsp/src/state.rs` — server state (cache, diagnostics)
- `crates/jdlint_lsp/src/handlers.rs` — onOpen, onChange, onSave, publishDiagnostics
- `extensions/jdlint-vscode/` — VS Code extension
- `.omc/docs/LSP_CACHING_DESIGN.md` — caching strategy (from M5.0)

---

### M6: Benchmarking & Performance Optimization

**Step M6.1.1: Benchmark harness**
- File: `benches/jfit_mobile_bench.rs`
- Criterion benchmarks: parse, analyze, total (jfit mobile lib)
- Variants: single-threaded, Rayon parallel
- Acceptance: benchmarks measure baseline performance

**Step M6.1.2: Profiling setup**
- Tools: perf, flamegraph (flamegraph crate)
- Activity: profile jdlint on jfit corpus; identify bottlenecks
- Acceptance: profiling data collected; bottlenecks identified

**Step M6.2: Optimization (if needed)**
- If performance >1000ms: apply targeted optimizations
- Examples: string interning, AST compression, per-rule parallelization
- Acceptance: performance <1000ms

---

### M7-M9: Nix Flake, Integration, Testing, Release

Covered above in milestone sections.

---

## 5. Expanded Test Plan (Deliberate Mode)

### 5.1 Unit Tests (Per Crate)

#### jdlint_dart_parser
- **Lexer**: 50 test cases (keywords, operators, strings, comments, multiline, interpolation)
- **Parser**: 100+ test cases (expressions, statements, declarations, types, patterns, error recovery)
- **AST round-trip**: parse → AST → pretty-print → reparse (optional, for robustness)
- **Target coverage**: >90% line coverage

#### jdlint_syntax
- **AST construction**: create nodes, verify structure
- **Span tracking**: spans correctly attributed
- **Visitor pattern**: walk AST, verify all nodes visited
- **Target coverage**: >85% line coverage

#### jdlint_analyze
- **Rule trait**: implement mock rule; verify trait contract
- **Visitor trait**: traverse sample AST; verify visitor contract
- **AnalyzeContext**: access config, add diagnostics, get spans
- **RuleRegistry**: register rules, lookup, run single rule
- **Parallel execution**: 10-file sample; verify thread safety (miri linter)
- **Target coverage**: >90% line coverage

#### jdlint_diagnostics
- **Diagnostic construction**: create, add message, add code, add suggestion
- **Serialization**: JSON, LSP, text formats; verify round-trip
- **Target coverage**: >85% line coverage

#### jdlint_config
- **Config deserialization**: valid, invalid, partial JSON
- **Default handling**: missing fields get defaults
- **Validation**: invalid field names cause errors or warnings
- **Target coverage**: >90% line coverage

#### jdlint_cli
- **Arg parsing**: clap validation, help output
- **File walker**: directory tree, exclude patterns
- **Output formatting**: JSON, text, quiet modes
- **Target coverage**: >85% line coverage

#### jdlint_rules (Per Rule)
- **Rule unit tests**: 5-10 cases per rule (violations, valid code, edge cases)
- **Configuration**: if rule has options, test configuration handling
- **Target coverage**: >90% per rule

#### jdlint_lsp
- **Protocol handling**: initialize, initialized, shutdown requests
- **File operations**: onOpen, onChange, onSave
- **Diagnostics publishing**: verify LSP format
- **State management**: cache invalidation, config reload
- **Target coverage**: >85% line coverage

---

### 5.2 Integration Tests

#### Parser + Syntax
- **Parse jfit sample code** (20 files): verify AST structure
- **Visitor pattern on AST**: traverse and collect all nodes
- **Error recovery**: parse code with syntax errors; verify recovery

#### Analyze + Rules
- **Mock rule on AST**: verify rule can run and emit diagnostics
- **Multiple rules in sequence**: verify rules don't interfere
- **Parallel rule execution**: 10 files, all rules; verify correctness

#### CLI + Config + Analyze
- **End-to-end check**: `jdlint check jfit_sample/` with config file
- **Output formats**: verify JSON and text outputs
- **Exit codes**: 0 for no errors, 1+ for errors

#### LSP + Analyze
- **Mock client workflow**: initialize → open file → modify → save
- **Incremental re-analysis**: modify single file; verify only that file re-analyzed
- **Config reload**: modify jdlint.json; verify rules reloaded
- **Concurrent files**: multiple files open; verify independence

#### Nix + CLI + Jfit
- **jdlint check on jfit mobile lib**: full 214-file corpus
- **Nix build jdlint**: produces binary
- **jfit devShell**: jdlint available on PATH

---

### 5.3 Snapshot Tests (Insta-style)

#### Parser
- **Lexer output**: token stream for 20 code samples
- **AST structure**: parsed AST for 20 code samples (JSON serialization)

#### Rules
- **Diagnostic output**: per-rule diagnostics for golden corpus (JSON or text)
- **Configuration**: config deserialization output

#### Diagnostics
- **Output formats**: JSON, LSP, text formats for sample diagnostics

#### CLI
- **Check output**: CLI output for 10-file sample (JSON and text)

#### LSP
- **Protocol messages**: initialize, open, change, save, diagnostics (JSON)

---

### 5.4 Corpus Tests (Real jfit Code)

#### Full jfit Mobile Lib
- **Parse**: all 214 files without panic
- **Analyze**: all rules run; collect diagnostics
- **Regression**: diagnostics match golden expectations (committed snapshots)
- **Performance**: jdlint check completes in <1000ms

#### Rule-Specific Corpus
- **Per-rule golden corpus**: 5-10 .dart files per rule
- **Expected diagnostics**: annotated in code (`/* expect: rule_name, line X */`)
- **Validation**: actual diagnostics match expected

---

### 5.5 Performance Tests / Benchmarks

#### Criterion Benchmarks
- **Parser**: single file, 50-file sample, full jfit corpus
- **Analyze**: per-stage (parser, rules, output)
- **Variants**: single-threaded baseline, Rayon parallel
- **Target**: <1000ms for full jfit corpus

#### Micro-Benchmarks
- **Per-rule**: measure time per rule on sample code
- **String interning**: compare performance with/without (if implemented)
- **AST traversal**: measure visitor pattern overhead

#### Flamegraph Profiling
- **Hot spots**: identify functions consuming >5% CPU
- **Memory profiling** (optional): track allocations, identify leaks

#### Regression Testing
- **Benchmark data committed**: any performance regression requires justification
- **CI**: benchmark runs on every push; alert on >5% regression

---

### 5.6 Observability & Health Metrics

#### Logging
- **Parser**: DEBUG logs for token stream, AST nodes
- **Analyze**: DEBUG logs for rule execution, diagnostics emitted
- **LSP**: DEBUG logs for client messages, file changes
- **CLI**: INFO logs for file discovery, analysis start/end

#### Metrics
- **Files processed**: count
- **Diagnostics emitted**: count, per severity
- **Execution time**: parser, analyze, output, total
- **Rules executed**: count, per rule
- **Memory usage**: peak RSS, allocation count (optional, valgrind/heaptrack)

#### Health Checks
- **Parser health**: successful parse rate (target: 100% on valid Dart)
- **Rule health**: diagnostic accuracy (golden corpus match rate)
- **LSP health**: response time to file changes (target: <100ms)
- **Performance**: analysis time (target: <1000ms for jfit)

---

### 5.7 Coverage Targets & Acceptance Gates (Explicit)

#### Coverage Targets Per Layer

| Layer | Target | Enforcement |
|-------|--------|-------------|
| Unit | >85% line coverage per crate | codecov CI report in M9.2 |
| Integration | 100% of major data flows tested (parser→AST→rules→diagnostics) | Integration test matrix in M8.4 |
| Snapshot | All outputs committed; >90% first-review acceptance rate | insta review in CI |
| Corpus | ≥5 positive + ≥5 negative examples per rule (300-600 files total) | M4.0 golden corpus audit |
| Performance | <1000ms end-to-end; <100ms per rule average | Criterion benchmark in M6 |

#### Acceptance Gates Per Milestone

| Milestone | Gate | Verification |
|-----------|------|-------------|
| M1.5 | Parser: 100% corpus parse success rate (214/214 files) | `cargo test --test corpus_tests` |
| M4.8 | Rules: ≥98% golden corpus match rate (all 60 rules) | `cargo xtask validate-rules` |
| M5.4 | LSP: <100ms incremental re-analyze per file | Criterion bench on M5.2 test |
| M6.3 | Performance: <1000ms end-to-end on jfit mobile | Criterion bench `jfit_mobile_bench` |
| M8.4 | Integration: all acceptance test suites pass | `cargo test --workspace --all-features` |
| M9.2 | Coverage: ≥85% line coverage per crate | codecov report |

---

## 6. ADR (Architecture Decision Record)

**Title:** Biome-Inspired Modular Workspace for Dart Linter (jdlint) Phase 1

**Status:** Accepted (Deliberate consensus, 2026-06-09)

### Decision

Build jdlint as a **Biome-inspired modular Rust workspace** with 8-10 semantic crates:
- `jdlint_dart_parser` (hand-rolled Dart 3.x recursive descent parser)
- `jdlint_syntax` (AST node types, visitor trait)
- `jdlint_analyze` (rule trait, registry, parallel execution)
- `jdlint_diagnostics` (diagnostic types, serialization)
- `jdlint_config` (jdlint.json deserialization)
- `jdlint_lsp` (Language Server Protocol server)
- `jdlint_cli` (command-line interface, file walker, output formatting)
- `jdlint` (binary entrypoint)
- `jdlint_analyze_macros` (optional, for boilerplate reduction)
- `xtask/codegen` (rule skeleton generation)

Phase 1 scope: port 60 rules (34 from dart_code_linter, 26 from pyramid_lint) enabled in jfit's analysis_options.yaml, no reformatting or check subcommand consolidation.

### Drivers

1. **Parallel Development**: 6-8 agents can work simultaneously on different crates (parser, rules, LSP, CLI) without merge conflicts.
2. **Testability**: Rule logic is isolated in `jdlint_rules`; rules are testable without full LSP or CLI stack.
3. **Scalability**: 60 rules in Phase 1; modular structure supports 100+ rules in Phase 2 without architectural rework.
4. **Proven Pattern**: Biome's multi-language architecture (94 crates, CSS, GraphQL, JSON, JavaScript, TypeScript) is evidence of design robustness.
5. **Phase 2 Preparation**: Parser isolation enables formatter and comprehensive lint reimplementation in Phase 2.

### Alternatives Considered

#### 1. Monolithic Single Crate (REJECTED)
- **Pros**: Simpler initial setup; faster "hello world" lint
- **Cons**: 
  - Rule implementations entangled with parser and LSP; no isolation
  - Difficult to parallelize rule execution
  - Testing requires full stack; slow iteration
  - Scales poorly to 100+ rules
- **Why Rejected**: Violates Principle 3; blocks parallel development; testing becomes intractable

#### 2. Full Biome Clone (90+ Crates) (REJECTED)
- **Pros**: Can borrow patterns, macros, infrastructure from Biome directly
- **Cons**:
  - Massive over-engineering; Dart is single-language, Biome is multi-language
  - Adds boilerplate: deserialize_macros, diagnostics_macros, control_flow, formatter, etc.
  - Version coordination overhead; larger CI/CD footprint
  - Timeline extended 2-3x
- **Why Rejected**: Complexity unjustified for Phase 1; risk of feature creep

#### 3. Hybrid: Tree-sitter Parser + Biome-like Rules (REJECTED)
- **Pros**: Leverage tree-sitter grammar; focus engineering on rules
- **Cons**:
  - Dependency on tree-sitter Dart grammar (quality/maintenance unknown)
  - AST mismatches between tree-sitter and dart_code_linter (source of diagnostic bugs)
  - No Dart SDK context; lost semantic information
  - Harder to port sophisticated rules (e.g., no-magic-number, which requires const evaluation)
- **Why Rejected**: Violates Principle 1 (hand-rolled parser); introduces dependency risk

### Why Option C (Selected) Was Chosen

**Option C balances engineering effort, parallel development, and long-term scalability:**

1. **Right-Sized Modularization**: 8-10 crates is enough to separate concerns (parser, rules, LSP, CLI) without Biome's complexity overhead.
2. **Parallel Development Enabler**: Different agents can work on parser (M1), rules (M4), LSP (M5), CLI (M3) simultaneously without blocking.
3. **Testing Isolation**: Rules can be unit-tested in `jdlint_rules` without instantiating CLI or LSP, enabling fast iteration.
4. **Performance**: Rayon parallelism in `jdlint_analyze` provides <1s on jfit corpus; single-language focus means no cross-language coordination.
5. **Phase 2 Ready**: Parser isolation (no formatting logic mixed in) enables Phase 2 formatter implementation without refactoring.
6. **Precedent**: Biome's design is proven; we're applying the same principles to a single language (Dart) with 70% fewer crates.

### Consequences

**Positive:**
- Parallel development enables 6-8 week timeline; single-crate approach would require 10-12 weeks
- Rule testability accelerates QA; golden corpus tests can run in isolation
- Modular structure de-risks large feature additions (formatter, comprehensive lint reimplementation)

**Negative:**
- More integration points to test; cross-crate version compatibility (mitigated by workspace members)
- Agents must understand trait boundaries (Rule, Visitor, AnalyzeContext); higher onboarding cost
- Larger test matrix; CI/CD runs longer (mitigated by parallel test execution in GitHub Actions)

**Risk Mitigations:**
- Clear documentation of trait contracts (Principles 3, 6)
- Regular integration checkpoints (end of M2, M4, M8) to catch cross-crate issues early
- Snapshot tests for regression detection
- Code review focus on trait boundaries and contract adherence

### Follow-Ups

1. **Phase 2 Planning**: Design formatter pipeline; determine if new crates needed (jdlint_dart_formatter, jdlint_semantic_analysis)
2. **Rule Macro Evaluation**: M4.1 decides whether `jdlint_analyze_macros` is justified; feedback will inform code generation strategy
3. **Performance Review**: M6 benchmarking data will identify if per-rule parallelization is needed or if Rayon work-stealing suffices
4. **Nix Optimization**: M7 integration may reveal flake.nix improvements for jfit (lockfile, caching)

---

## 7. Risk Register

| # | Risk | Probability | Impact | Mitigation | Owner | Status |
|---|------|-------------|--------|------------|-------|--------|
| **R1** | Parser correctness bug on real jfit code surfaces late (week 5-6) | MEDIUM | CRITICAL | Corpus-driven TDD in M2-M3; full jfit parse by end M3; fuzzing in M7 | Parser Engineer (Agent 1) | ACTIVE |
| **R2** | Rule porting semantics mismatch (e.g., no-magic-number thresholds off) | MEDIUM | HIGH | Rule spec audit (M4.0); golden test corpus; diff-based validation vs. original linter | Rule Engineers (Agents 2-3) | ACTIVE |
| **R3** | Performance regression: >1.2s on jfit corpus | LOW | HIGH | Benchmark harness in M3; micro-benchmarks in M5; profiling in M6 | Performance Engineer (Agent dedicated to M6-M7) | ACTIVE |
| **R4** | LSP server hangs or crashes under concurrent file edits | MEDIUM | HIGH | Incremental analysis testing (M5.2); concurrent file tests (M8.3); miri linter for thread safety | LSP Engineer (Agent 4) | ACTIVE |
| **R5** | CI/CD failures due to Nix flake complexity or jfit flake integration | MEDIUM | MEDIUM | Nix expert review (M7); jfit flake changes tested in isolation (M7.2) | DevOps / Nix Engineer | ACTIVE |
| **R6** | Agent onboarding overhead; trait boundaries not understood | MEDIUM | MEDIUM | Clear documentation of trait contracts (Principles 3, 6); regular sync meetings; code review focus on boundaries | Planner / Architect | ACTIVE |
| **R7** | Test coverage gaps; golden corpus tests incomplete for some rules | LOW | MEDIUM | Golden corpus audit (M4.0); coverage reports in M9.2; >80% line coverage requirement | QA Engineer | ACTIVE |
| **R8** | Workspace version conflicts; inter-crate incompatibility | LOW | MEDIUM | Workspace members (single version per crate); regular integration tests (M8.1-M8.3) | Build Engineer | ACTIVE |
| **R9** | Jfit codebase has undocumented Dart syntax not covered by parser | MEDIUM | HIGH | Extend parser grammar for edge cases; corpus validation (M1.4, M8.1) | Parser Engineer (Agent 1) | ACTIVE |
| **R10** | Rule implementations don't pass code review due to style/efficiency issues | LOW | MEDIUM | Clippy pass (M9.2); code review SLA (24 hours); rework loop if issues found | Architect / Code Reviewer | ACTIVE |

**Risk Response Strategy:**
- **Active monitoring**: Weekly sync meetings to track milestones and emerging risks
- **Early escalation**: If R1-R3 (critical/high impact) show signs of slipping, escalate to team lead immediately
- **Contingency planning**: If parser bugs or performance issues emerge in M3, allocate extra agents to M6 optimization sprint
- **Fallback**: If <1s target appears unachievable, pivot to streaming diagnostics (emit as parsing completes) as alternative

---

## 8. Open Questions & Deferred Decisions

Tracked in `.omc/plans/open-questions.md`:

1. **Rule specification clarity**: Should we reverse-engineer specs from dart_code_linter source, or reach out to original authors for official specs? (Affects M4.0 timeline)
2. **Auto-fix scope**: Phase 1 explicitly excludes auto-fix for most rules. Should we identify "safe" subset (e.g., trailing-comma, format-comment) for Phase 1? (Deferred to Phase 2, but affects CLI design)
3. **VS Code extension publishing**: Should jdlint-vscode extension be published to VS Code Marketplace, or distributed as part of jfit toolchain? (Affects M5.3 and release strategy)
4. **Fuzzing in M7**: Is libFuzzer integration worth the time investment for parser validation? Or is corpus validation sufficient? (Optional, contingent on M3 performance)
5. **Semantic analysis for complex rules**: Rules like no-magic-number and no-object-declaration require const evaluation or type inference. Should we build a simplified semantic analyzer, or keep rules AST-only? (Architectural decision for M4)

---

## 9. Summary: What Gets Built

### Phase 1 Deliverables

1. **Rust Workspace**
   - Cargo.toml with 9 crates + xtask/codegen
   - Full Dart 3.x hand-rolled parser
   - 60 lint rules (34 from dart_code_linter, 26 from pyramid_lint)
   - LSP server compliant with Language Server Protocol
   - CLI with check, lsp, version subcommands
   - jdlint.json config file (biome.json-style)

2. **Testing & Validation**
   - 300+ unit tests (>85% line coverage per crate)
   - 600+ golden corpus tests (real jfit .dart files)
   - Performance benchmarks (Criterion); target <1000ms on 214 jfit files
   - CI/CD pipeline (GitHub Actions)
   - Snapshot tests for parser, rules, diagnostics, CLI output

3. **Documentation**
   - README (installation, usage, configuration)
   - ARCHITECTURE.md (design overview, crate responsibilities, trait contracts)
   - CONTRIBUTING.md (how to add rules, testing expectations)
   - Rule specification document (M4.0 output)
   - grammar.md (Dart grammar reference for parser)

4. **Nix & Integration**
   - jdlint/flake.nix (jdlint build derivation)
   - Updated jfit/flake.nix (jdlint as input; binary in devShell)
   - Binary available in jfit devShell via `nix develop`

5. **VS Code Extension**
   - Minimal extension (language client)
   - Connects to jdlint LSP server
   - Displays diagnostics in editor

### Acceptance Criteria (All Must Pass)

- [x] Repo renamed to jdlint; Cargo.toml name = "jdlint"
- [x] cargo build --release compiles clean with zero warnings (Rust stable)
- [x] Dart 3.x parser round-trips all 214 jfit .dart files without parse errors
- [x] All 60 ported rules produce correct diagnostics (golden corpus match)
- [x] jdlint check . completes in <1000ms on jfit mobile project
- [x] LSP server starts and VS Code shows jdlint diagnostics for .dart files
- [x] jdlint.json config controls rule enable/disable
- [x] nix build produces jdlint binary
- [x] jfit/flake.nix updated; jdlint available in devShell
- [x] nix develop in jfit makes jdlint available on PATH

---

## 10. Implementation Team & Workload Estimate

**Recommended team structure:**

| Role | Crates | Weeks | Hours | Notes |
|------|--------|-------|-------|-------|
| **Parser Engineer** (Agent 1) | jdlint_dart_parser, jdlint_syntax | 2-3 | 100-120 | M1, corpus tests, fuzzing (optional) |
| **Rule Engineers** (Agents 2-6, 5 total) | jdlint_rules | 2-3 | 210-330 (parallel) | M4: 10 rules per agent, 35-55 hrs each |
| **Infrastructure Engineer** (Agent 7) | jdlint_analyze, jdlint_diagnostics, jdlint_config | 2 | 80-100 | M2, rule trait design, test harness |
| **CLI/LSP Engineer** (Agent 8) | jdlint_cli, jdlint_lsp, VS Code extension | 2-3 | 80-100 | M3, M5, CLI, LSP, incremental analysis |
| **Performance Engineer** | Benchmarking, optimization, profiling | 1 | 40-60 | M6, M7, optimization sprints |
| **QA/Integration Engineer** | Corpus tests, acceptance tests, CI/CD | 2-3 | 60-80 | M8, regression testing, release prep |
| **Architect / Code Reviewer** | Design review, trait contracts, ADR | 2-3 | 60-80 | Ongoing, M2/M4/M8 checkpoints |
| **Planner / Project Manager** | Planning, coordination, risk management | 2 | 40-60 | Coordination, status tracking |

**Revised Total: ~750-950 hours over 9-11 weeks** (added M0.5, M2.5, M5.0, pre-mortem work ~40 hours)  
**8-person team: ~85-110 hours per person per week (aggressive but achievable with clear task boundaries)**

**Timeline Breakdown:**
- Week 1 (Days 1-5): M0 (4-6h) + M0.5 (28-36h) = 32-42h
- Weeks 2-3 (Days 4-11): M1 (80-100h)
- Weeks 3-4 (Days 11-16): M2 (80-120h including M2.5)
- Weeks 4-5 (Days 17-23): M3 (40-60h)
- Weeks 6-7 (Days 24-37): M4 (340-440h including M4.0 spec audit)
- Weeks 7-9 (Days 39-54): M5 (80-120h including M5.0 design)
- Weeks 9-10 (Days 51-60): M6 (40-80h, conditional on performance)
- Weeks 10-11 (Days 57-64): M7 (20-30h)
- Week 11 (Days 64-70): M8 (60-80h)
- Weeks 11-12 (Days 70-77): M9 (40-60h)

Total calendar time: 11-12 weeks (77 days)

---

## Approval & Sign-Off

**Plan Status:** Ready for Architect review and team execution  
**Next Step:** Architect validates design decisions and trait contracts; Critic identifies missing requirements or risks; Executor begins M0 (project setup) on approval

**Questions or adjustments before proceeding to implementation?**

