# Parallelism Model (Phase 1): Per-File Rayon Strategy

**Audience**: M2 (analyze infrastructure), M6 (optimization)  
**Phase**: Phase 1 (MVP: <1s on 214 jfit files)  
**Last Updated**: 2026-06-09  
**Status**: Design locked for M2.2 implementation

---

## 1. Decision: Per-File Parallelism (Phase 1 Selected)

### Model
Each `.dart` file is one Rayon work unit. All rules run **sequentially** within that file's task.

```
┌─────────────────────────────────────────┐
│ Input: files: &[&Path] (214 .dart files) │
└──────────────┬──────────────────────────┘
               │
       rayon::par_iter()
               │
    ┌──────────┼──────────┬──────────┐
    │          │          │    ...   │
    ▼          ▼          ▼          ▼
  file1      file2      file3   file214
    │          │          │          │
 read+parse  read+parse read+parse read+parse
    │          │          │          │
 rule1       rule1      rule1     rule1
 rule2       rule2      rule2     rule2
 rule3       rule3      rule3     rule3
    │          │          │          │
    └──────────┼──────────┼──────────┘
               │
        collect all
        diagnostics
```

### Why This Strategy Was Selected

**File count + typical file size = near-linear parallelism**
- jfit corpus: 214 files
- File size range: 200–800 lines (median ~400)
- Single-file analysis time: estimated 5–20ms per file (measured at M1.5)
- Expected wall-clock time: 214 files × 20ms / N CPUs ≈ 4.3s / N
- On 8 CPUs: ~540ms; on 4 CPUs: ~1.1s → target achievable

**Per-rule parallelism (rejected)**
- Adds Rayon overhead (task spawning, work stealing) for minimal gain
- Requires `Arc<Mutex<Vec<Diagnostic>>>` as shared sink → lock contention
- Useful only if single-file analysis >100ms (unlikely at 5–20ms)
- Deferred to Phase 2 contingency (Section 5)

**Single-threaded (rejected)**
- 214 files × 20ms = 4.3s → misses <1s target by 4×
- No option without parallelism

### Phase 1 Constraint
**RuleVisitor implementations MUST NOT use mutable self state.** This is enforced by the `&self` receiver on `analyze()`. Weaken to `&mut self` only if per-rule parallelism is triggered (M6.2 gate).

---

## 2. Architecture: `RuleRegistry::analyze_parallel`

### Public API (M2.2 implementation contract)

```rust
impl RuleRegistry {
    /// Analyze multiple files in parallel using Rayon.
    ///
    /// # Arguments
    /// - `files`: slice of file paths to analyze
    /// - `config`: lint configuration (shared across all rules)
    ///
    /// # Returns
    /// Flat vector of diagnostics from all files and rules.
    /// Order is not guaranteed due to parallelism.
    pub fn analyze_parallel(
        &self,
        files: &[&Path],
        config: &JdlintConfig,
    ) -> Vec<Diagnostic> {
        // TODO: M2.2 — implement using rayon::par_iter()
    }
}
```

### Implementation Pattern (Pseudocode)

```rust
use rayon::prelude::*;
use std::path::Path;

impl RuleRegistry {
    pub fn analyze_parallel(
        &self,
        files: &[&Path],
        config: &JdlintConfig,
    ) -> Vec<Diagnostic> {
        files
            .par_iter()
            .flat_map(|file_path| {
                // Each parallel task: read + parse + analyze one file
                self.analyze_file(file_path, config)
            })
            .collect()
    }

    /// Sequential analysis of one file (runs in Rayon task).
    fn analyze_file(
        &self,
        file_path: &Path,
        config: &JdlintConfig,
    ) -> Vec<Diagnostic> {
        // 1. Read source code
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => return vec![], // TODO: M2.2 — error handling strategy
        };

        // 2. Parse AST
        let program = match jdlint_syntax::parse(&source) {
            Ok(p) => p,
            Err(_) => return vec![], // TODO: M2.2 — error handling strategy
        };

        // 3. Create per-file context (immutable refs)
        let ctx = AnalyzeContext {
            file_path,
            source: &source,
            config,
        };

        // 4. Run all rules sequentially on this file
        let mut diagnostics = Vec::new();
        for rule in &self.rules {
            let rule_diags = rule.analyze(&program, &ctx);
            diagnostics.extend(rule_diags);
        }

        diagnostics
    }
}
```

### Key Design Points

**No shared mutable state**
- Each Rayon task gets its own `source` (owned `String`)
- Each task gets its own `program` (owned `Program`)
- Each task accumulates diagnostics in a local `Vec<Diagnostic>`
- `AnalyzeContext` contains only shared refs (`&Path`, `&str`, `&JdlintConfig`)
- Rayon's `flat_map()` collects per-task results without contention

**Rules share immutably**
- `self.rules: &[Box<dyn Rule>]` passed to each task as `&self`
- Each rule reference is `Send + Sync` (enforced by trait bound)
- No mutation: `rule.analyze(&program, &ctx)` takes `&self`, not `&mut self`

**File I/O outside parallel boundary**
- Rayon does not parallelize file I/O within the closure
- Each task does: read (blocking I/O) → parse → analyze
- File I/O is parallelizable; parse/analyze scales with CPU count

---

## 3. Thread Safety Requirements

### What Is Required for Correctness

**Trait bounds**
```rust
pub trait Rule: Send + Sync {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>;
}
```
- `Send`: Rule instance can be safely moved between threads (ensured by Rayon work stealing)
- `Sync`: Rule instance can be safely shared via `&self` across threads
- **Enforcement**: compiler rejects non-`Send` or non-`Sync` fields in Rule implementations

**RuleVisitor (immutability)**
- No mutable self fields in Rule implementations
- `analyze(&self, ...)` takes immutable borrow → cannot mutate internal state
- This is **compile-time enforced** by Rust's borrow checker

**AnalyzeContext (all refs are Send + Sync)**
```rust
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,  // Send + Sync
    pub source: &'a str,                  // Send + Sync
    pub config: &'a JdlintConfig,         // TODO: M2.2 — verify JdlintConfig is Send + Sync
}
```

**Diagnostic result (no sharing required)**
- Each task returns `Vec<Diagnostic>` (owned value)
- No Arc, no Mutex, no RefCell needed
- Rayon's `flat_map()` collects into a single result vec without locks

### What Would Break Thread Safety (Forbidden)

**Rule caching via Mutex/RefCell**
```rust
// ❌ FORBIDDEN
pub struct MyRule {
    cache: Mutex<HashMap<String, Result>>,
}

impl Rule for MyRule {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut cache = self.cache.lock().unwrap();  // Lock contention under parallelism
        // ...
    }
}
```
- Creates bottleneck: all threads contend on single lock
- Phase 1 design: compute fresh on every file, no caching

**Mutable global state**
```rust
// ❌ FORBIDDEN
static mut GLOBAL_BUFFER: Vec<Diagnostic> = Vec::new();

impl Rule for MyRule {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        unsafe { GLOBAL_BUFFER.push(...); }  // Data race
    }
}
```
- Undefined behavior under parallelism

**Rule spawning additional threads**
```rust
// ❌ FORBIDDEN
impl Rule for MyRule {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let handle = std::thread::spawn(|| { ... });  // Nested parallelism
        // ...
    }
}
```
- Rayon oversubscribes CPU → performance collapse

---

## 4. Baseline Measurement Plan (M1.5)

### Purpose
Validate that per-file parallelism meets <1s target. If single-file analysis >100ms, trigger contingency (Section 5).

### Benchmark Harness
```
crates/jdlint_dart_parser/benches/parse_bench.rs
```

Measure these operations separately:
1. **Parse time**: `jdlint_syntax::parse()` only
2. **Analyze time**: all rules on parsed AST
3. **Total time**: read + parse + analyze

### Test Corpus
- 50 representative `.dart` files from jfit project
- Include: small files (100 lines), medium (400 lines), large (800 lines)
- Cover: leaf modules, deep nesting, generic code

### Measurement Report Format

```
file_path                          | parse_ms | analyze_ms | total_ms
───────────────────────────────────┼──────────┼────────────┼─────────
lib/main.dart                      |    2.5   |    8.3     |   10.8
lib/models/user.dart               |    1.8   |    6.2     |    8.0
lib/screens/home_screen.dart       |    3.2   |   12.1     |   15.3
...
───────────────────────────────────┴──────────┴────────────┴─────────
Median total time:                 |          |            |   11.2
95th percentile:                   |          |            |   18.5
Max (single file):                 |          |            |   22.1
```

### Success Criteria

| Metric | Threshold | Outcome |
|--------|-----------|---------|
| Median single-file analysis | <100ms | ✓ Continue Phase 1 |
| Max single-file analysis | <150ms | ✓ Continue Phase 1 |
| Median all 214 files on 1 CPU | <4.3s | ✓ Linear extrapolation valid |
| If any threshold exceeded | — | → Trigger contingency |

### Tool

```bash
cargo bench --bench parse_bench -- --verbose
```

Requires: Criterion crate (`benches/Cargo.toml`)

### Reporting (M1.5 deliverable)

Document to: `.omc/research/M1.5_BASELINE_MEASUREMENT.md`
- Include: chart of parse_ms vs. file size (lines of code)
- Include: chart of analyze_ms vs. file size
- Include: pass/fail against thresholds
- Decision: "Per-file parallelism approved for M2.2" or "Trigger contingency"

---

## 5. Contingency: Per-Rule Parallelism Switch

### Trigger Condition
If M1.5 baseline measurement shows:
- Median single-file analysis >100ms, **OR**
- Max single-file analysis >150ms

### If Triggered (M6.2 decision gate)

**Architecture change:**
- Rules within same file run in parallel via `rayon::scope()`
- Each rule gets its own Rayon task

**Implementation sketch:**
```rust
impl RuleRegistry {
    pub fn analyze_parallel(
        &self,
        files: &[&Path],
        config: &JdlintConfig,
    ) -> Vec<Diagnostic> {
        files
            .par_iter()
            .flat_map(|file_path| {
                self.analyze_file_per_rule_parallel(file_path, config)
            })
            .collect()
    }

    fn analyze_file_per_rule_parallel(
        &self,
        file_path: &Path,
        config: &JdlintConfig,
    ) -> Vec<Diagnostic> {
        let source = std::fs::read_to_string(file_path).unwrap_or_default();
        let program = jdlint_syntax::parse(&source).unwrap_or_else(|_| {
            Program::default() // TODO: M6.2 — error handling
        });

        let ctx = AnalyzeContext {
            file_path,
            source: &source,
            config,
        };

        let diag_sink = Arc::new(Mutex::new(Vec::new()));

        // TODO: M6.2 — rayon::scope to parallelize rules
        rayon::scope(|s| {
            for rule in &self.rules {
                let sink = Arc::clone(&diag_sink);
                s.spawn(move |_| {
                    let diags = rule.analyze(&program, &ctx);
                    sink.lock().unwrap().extend(diags);
                });
            }
        });

        Arc::try_unwrap(diag_sink)
            .unwrap()
            .into_inner()
            .unwrap()
    }
}
```

**Required preemptive design change (enforce NOW at M2.2):**
- RuleVisitor **MUST NOT** mutate internal state
- Enforced by: `&self` receiver (compile-time)
- **Do not weaken to `&mut self`** unless contingency is triggered

**Lock strategy for shared sink:**
- `Arc<Mutex<Vec<Diagnostic>>>` as diagnostic accumulator
- Each rule: `sink.lock().unwrap().extend(rule_diags)`
- Lock is held only during extend (minimal contention)

**Decision gate:**
- M6.2 profiling report (from M1.5 baseline) reviewed by Architect
- Gate: do not merge M6.2 per-rule parallelism without explicit approval
- If not triggered: delete contingency code from M6.2

---

## 6. Future: Incremental Analysis (Phase 2)

### LSP Watch Mode Integration

When adding LSP incremental analysis (Phase 2 or later):

**File-level granularity is preserved:**
- Re-analyze only files that changed (e.g., on file save)
- Per-file parallelism applies to changed file set
- If 1 file changed: 1 task spawned (no parallelism, but fast)
- If 50 files changed: 50 tasks in parallel (scales with CPU count)

**Rule instances are reused:**
- Create `RuleRegistry` once (at LSP startup)
- Reuse same rules across multiple file re-analyses
- Immutability makes this safe: no state accumulates between runs

**Performance implication:**
- Incremental is "free" if per-file analysis is <50ms (amortized over 214 files)
- If incremental batches 10 changed files, Rayon parallelism still applies to the batch
- Baseline measurement (M1.5) directly informs LSP latency budget

---

## 7. Implementation Checklist (M2.2)

- [ ] Add `rayon` crate to `crates/jdlint_analyze/Cargo.toml`
- [ ] Implement `RuleRegistry::analyze_parallel()` per Section 2 pattern
- [ ] Implement `RuleRegistry::analyze_file()` helper (sequential)
- [ ] Verify all `Rule` implementations are `Send + Sync` (compiler check)
- [ ] Verify `AnalyzeContext` fields are `Send + Sync`
- [ ] Verify `JdlintConfig` is `Send + Sync` (if not, adjust AnalyzeContext)
- [ ] Add error handling for read/parse failures (return empty diagnostics or error diagnostic)
- [ ] Add integration test: `test_analyze_parallel_matches_sequential()` (verify correctness)
- [ ] Document: RuleRegistry::analyze_parallel in crate-level docs
- [ ] Defer to M1.5: baseline measurement (Section 4)
- [ ] Do NOT use per-rule parallelism unless M1.5 triggers contingency

---

## 8. Reference: AnalyzeContext & Rule Definitions

From source code:

**AnalyzeContext** (`crates/jdlint_analyze/src/context.rs`):
```rust
pub struct AnalyzeContext<'a> {
    pub file_path: &'a std::path::Path,
    pub source: &'a str,
    pub config: &'a JdlintConfig,
}
```

**Rule trait** (`crates/jdlint_analyze/src/rule.rs`):
```rust
pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>;
}
```

**RuleRegistry** (`crates/jdlint_analyze/src/registry.rs`):
```rust
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleRegistry {
    pub fn new() -> Self { /* ... */ }
    pub fn register(&mut self, rule: Box<dyn Rule>) { /* ... */ }
}
```

---

## Approval History

| Phase | Decision | Approver | Date |
|-------|----------|----------|------|
| Design | Per-file parallelism selected | — | 2026-06-09 |
| M1.5 | Baseline measurement complete | — | *pending* |
| M6.2 | Contingency gate (if triggered) | Architect | *pending* |

