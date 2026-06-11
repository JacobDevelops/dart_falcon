# LSP Caching & Incremental Analysis Design (M5.0)

**Status:** Locked for Phase 1 (M5 implementation)
**Date:** 2026-06-11
**Scope:** `falcon_lsp` crate — server loop, document cache, config reload, debouncing

---

## 1. Architecture Overview

The LSP server is a **single-threaded message loop** over `lsp-server`'s
`Connection` (crossbeam channels). All analysis is sequential within the loop
thread — per the LSP spec there is no concurrent file analysis in LSP mode.
Rayon is not used here; single-file analysis is ~1ms on jfit-sized files, so
parallelism would add complexity without benefit (M0.5 parallelism model,
contingency not triggered).

```
stdin ──► lsp-server reader thread ──► crossbeam channel ──► server loop
                                                              │  LspState
stdout ◄─ lsp-server writer thread ◄── crossbeam channel ◄────┘  (single thread)
```

Transport: stdio (`falcon lsp`). Tests use `Connection::memory()` with the
same loop (`run_with_connection`), so the protocol path is identical in tests
and production.

## 2. `LspState` — the cache

```rust
struct DocumentState {
    text: String,              // full text (FULL sync; see §5)
    version: Option<i32>,      // client-reported document version
    program: Program,          // cached AST — rebuilt only on text change
    last_diagnostics: Vec<falcon_diagnostics::Diagnostic>, // byte-span diags (hover)
    parse_count: u64,          // instrumentation (incremental tests)
    analyze_count: u64,        // instrumentation (incremental tests)
}

pub struct LspState {
    documents: HashMap<String /* uri */, DocumentState>,
    config: FalconConfig,
    config_path: Option<PathBuf>, // explicit path or None → discovery from cwd
    rules: Vec<Box<dyn Rule>>,    // enabled set, rebuilt on config reload
}
```

**Two independent cache axes — this is the core invariant:**

| Event | AST (`program`) | Rules (`rules`) | Re-analysis scope |
|-------|-----------------|-----------------|-------------------|
| `didOpen` | parse new | unchanged | opened file only |
| `didChange` | re-parse **that file only** | unchanged | changed file only |
| `didSave` | re-parse if text included, else reuse | unchanged | saved file only |
| `didClose` | dropped | unchanged | none (publish empty) |
| config change | **reused for all open files** | rebuilt from new config | all open files |

The stale-AST-with-new-config bug is avoided by construction: an AST is
invalidated by *text* changes only, and the rule set is invalidated by
*config* changes only. A config reload re-runs the (new) rules over the
(still-valid) cached ASTs — no re-parse, no stale results. Diagnostics are
always recomputed from `(program, rules)` at publish time; `last_diagnostics`
is a *copy of the most recent output* (for hover), never an input.

## 3. Cache invalidation triggers

1. **`textDocument/didChange`** (file content) — replace `text`, re-parse,
   re-analyze that document, publish. Debounced (§4).
2. **`workspace/didChangeWatchedFiles`** (falcon.json) — reload config via
   `falcon_config` (explicit path if the server was started with one,
   otherwise discovery from cwd), rebuild the enabled-rule set with
   `falcon_rules::enabled_rules`, re-analyze **all open documents** against
   cached ASTs, publish each. The VS Code client watches `**/falcon.json`
   (languageclient `synchronize.fileEvents`); no dynamic registration needed
   in Phase 1.
3. **`textDocument/didClose`** — drop the document entry and publish an empty
   diagnostic set so the editor clears stale squiggles.

## 4. Debouncing

Rapid `didChange` bursts (typing) must not trigger per-keystroke analysis.

- Strategy: **trailing-edge debounce, 500ms** (plan M5.0). On `didChange` the
  text is applied to the cache immediately, the document is marked dirty, and
  a deadline `now + debounce` is (re)set.
- The loop uses `recv_timeout(deadline - now)` instead of blocking `recv()`
  while any document is dirty. On timeout, all dirty documents are analyzed
  and published in one flush.
- `didOpen` / `didSave` / config reload analyze **immediately** (they are not
  burst events) and clear the document's dirty flag.
- The debounce duration is a `ServerOptions` field; tests run with
  `Duration::ZERO` (flush immediately) except the dedicated debounce test.

## 5. Sync mode

**Full text sync** (`TextDocumentSyncKind::FULL`). Incremental *range* sync is
a Phase 2 concern: the parser has no incremental reparse, files are small
(<2k lines in jfit), and a full re-parse is sub-millisecond. `didSave` is
registered with `include_text: true` as a defensive refresh.

## 6. Thread safety

- The server loop owns `LspState` exclusively — no shared mutable state, no
  locks. The only threads are lsp-server's reader/writer I/O threads, which
  communicate via channels.
- Rule instances are immutable (`Rule: Send + Sync`, enforced at M2.2);
  reusing one `Vec<Box<dyn Rule>>` across analyses is safe by contract.

## 7. Performance target

M5.4 / M8.3 gate: **<100ms single-file incremental re-analyze** (change →
diagnostics computed). Expected actual: <5ms (full 214-file jfit corpus
analyzes in ~26ms). Enforced by a timing assertion in
`falcon_lsp/tests/incremental_tests.rs`.

## 8. Out of scope (Phase 2)

- Incremental (range-based) text sync and incremental reparse
- Workspace-wide diagnostics for unopened files
- Code actions / auto-fix (`textDocument/codeAction`)
- Dynamic capability registration for watched files
- Cross-file semantic caching (no cross-file analysis exists in Phase 1)
