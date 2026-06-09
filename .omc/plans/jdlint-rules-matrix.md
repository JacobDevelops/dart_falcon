# jdlint Rules Matrix

**Date:** 2026-06-09  
**Phase:** Phase 1 — Port rules from dart_code_linter + pyramid_lint  
**Status:** Scaffold — complexity tiers assigned in M0.5.3

---

## Crate Architecture

| Crate | Responsibility | Phase |
|-------|---------------|-------|
| `jdlint_dart_parser` | Dart 3.x lexer + recursive-descent parser → AST | M1 |
| `jdlint_syntax` | AST node types, token types, syntax tree | M1 |
| `jdlint_analyze` | `Rule` trait, `RuleVisitor`, `AnalyzeContext`, Rayon engine | M2 |
| `jdlint_rules` | All ported lint rules (dart_code_linter + pyramid_lint) | M4 |
| `jdlint_diagnostics` | `Diagnostic`, `Severity`, `Span` types | M2 |
| `jdlint_config` | `jdlint.json` schema, config loader | M3 |
| `jdlint_cli` | CLI args (clap), `check` + `lsp` commands | M3 |
| `jdlint_lsp` | LSP 3.17 server over JSON-RPC 2.0 | M5 |
| `jdlint` (binary) | Entry point; wires CLI → analyze → diagnostics | M3 |
| `xtask` | Codegen: rule visitor stubs, test scaffolding | M2 |

---

## Rule Sources

Rules are ported from two sources as enabled in jfit's `analysis_options.yaml`.

### dart_code_linter Rules (~34 rules)

| Rule | Complexity | Semantic Tag | Owning Module |
|------|-----------|-------------|---------------|
| `avoid-dynamic` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-global-state` | MEDIUM | AST-only | `dart_code_linter` |
| `avoid-ignoring-return-values` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-late-keyword` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-nested-conditional-expressions` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-non-null-assertion` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-passing-async-when-sync-expected` | MEDIUM | requires-scope-lookup | `dart_code_linter` |
| `avoid-redundant-async` | MEDIUM | AST-only | `dart_code_linter` |
| `avoid-returning-widgets` | MEDIUM | requires-scope-lookup | `dart_code_linter` |
| `avoid-throw-in-catch-block` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-top-level-member-access` | SIMPLE | AST-only | `dart_code_linter` |
| `avoid-unnecessary-type-assertions` | MEDIUM | requires-type-inference | `dart_code_linter` |
| `avoid-unnecessary-type-casts` | MEDIUM | requires-type-inference | `dart_code_linter` |
| `avoid-unrelated-type-assertions` | MEDIUM | requires-type-inference | `dart_code_linter` |
| `avoid-unused-parameters` | MEDIUM | requires-scope-lookup | `dart_code_linter` |
| `binary-expression-operand-order` | SIMPLE | AST-only | `dart_code_linter` |
| `double-literal-format` | SIMPLE | AST-only | `dart_code_linter` |
| `member-ordering` | COMPLEX | AST-only | `dart_code_linter` |
| `no-boolean-literal-compare` | SIMPLE | AST-only | `dart_code_linter` |
| `no-empty-block` | SIMPLE | AST-only | `dart_code_linter` |
| `no-equal-arguments` | SIMPLE | AST-only | `dart_code_linter` |
| `no-equal-then-else` | SIMPLE | AST-only | `dart_code_linter` |
| `no-magic-number` | COMPLEX | requires-const-eval | `dart_code_linter` |
| `no-object-declaration` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-async-await` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-conditional-expressions` | MEDIUM | AST-only | `dart_code_linter` |
| `prefer-const-border-radius` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-correct-edge-insets-constructor` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-correct-identifier-length` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-extracting-callbacks` | MEDIUM | AST-only | `dart_code_linter` |
| `prefer-first` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-immediate-return` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-last` | SIMPLE | AST-only | `dart_code_linter` |
| `prefer-trailing-comma` | MEDIUM | AST-only | `dart_code_linter` |

### pyramid_lint Rules (~26 rules)

| Rule | Complexity | Semantic Tag | Owning Module |
|------|-----------|-------------|---------------|
| `avoid_abbreviations_in_doc_comments` | SIMPLE | AST-only | `pyramid_lint` |
| `avoid_empty_blocks` | SIMPLE | AST-only | `pyramid_lint` |
| `avoid_inverted_boolean_expressions` | SIMPLE | AST-only | `pyramid_lint` |
| `avoid_mutable_global_variables` | MEDIUM | AST-only | `pyramid_lint` |
| `avoid_nested_if` | SIMPLE | AST-only | `pyramid_lint` |
| `avoid_positional_fields_in_records` | SIMPLE | AST-only | `pyramid_lint` |
| `avoid_unused_parameters` | MEDIUM | requires-scope-lookup | `pyramid_lint` |
| `boolean_prefixes` | SIMPLE | AST-only | `pyramid_lint` |
| `class_members_ordering` | COMPLEX | AST-only | `pyramid_lint` |
| `correct_order_for_super_dispose` | SIMPLE | AST-only | `pyramid_lint` |
| `max_lines_for_file` | SIMPLE | AST-only | `pyramid_lint` |
| `max_lines_for_function` | SIMPLE | AST-only | `pyramid_lint` |
| `max_parameters_for_function` | SIMPLE | AST-only | `pyramid_lint` |
| `max_switch_cases` | SIMPLE | AST-only | `pyramid_lint` |
| `no_duplicate_case_values` | SIMPLE | AST-only | `pyramid_lint` |
| `no_empty_block` | SIMPLE | AST-only | `pyramid_lint` |
| `no_magic_number` | COMPLEX | requires-const-eval | `pyramid_lint` |
| `prefer_declaring_const_constructor` | SIMPLE | AST-only | `pyramid_lint` |
| `prefer_dedicated_media_query_methods` | MEDIUM | AST-only | `pyramid_lint` |
| `prefer_iterable_any` | SIMPLE | AST-only | `pyramid_lint` |
| `prefer_iterable_every` | SIMPLE | AST-only | `pyramid_lint` |
| `prefer_underscore_for_unused_callback_parameters` | SIMPLE | AST-only | `pyramid_lint` |
| `unnecessary_flutter_imports` | MEDIUM | requires-scope-lookup | `pyramid_lint` |
| `unnecessary_nullable_return_type` | MEDIUM | requires-type-inference | `pyramid_lint` |
| `use_once_constructors_once_provider` | COMPLEX | requires-scope-lookup | `pyramid_lint` |
| `use_spacer_as_expanded_child` | SIMPLE | AST-only | `pyramid_lint` |

---

## Complexity Distribution

| Tier | Count | Estimated Hours | Implementation Window |
|------|-------|-----------------|-----------------------|
| SIMPLE | ~40 | 40–80h (1–2h each) | M4.2–M4.4 (parallel) |
| MEDIUM | ~14 | 28–42h (2–3h each) | M4.4–M4.5 |
| COMPLEX | ~6 | 24–36h (4–6h each) | M4.5–M4.6 |
| **Total** | **~60** | **92–158h** | **M4.2–M4.6** |

*Note: Exact counts and tiers finalized in M0.5.3 (Rule Analysis Matrix) after reviewing jfit's analysis_options.yaml and reverse-engineering source behavior.*

---

## Semantic Complexity Notes

- **`requires-type-inference`**: Phase 1 uses simplified heuristics (pattern-match on AST shape; no type resolution). Accuracy ~80–90%; full type inference deferred to Phase 2.
- **`requires-const-eval`**: `no-magic-number` in Phase 1 bans all numeric literals except `0`, `1`, `-1` (simplified heuristic). Full const evaluation deferred to Phase 2.
- **`requires-scope-lookup`**: Phase 1 uses best-effort lexical scope from AST. No symbol table; cross-file resolution deferred to Phase 2.

---

## M0.6 Pre-Flight Checklist

Verified before M4 rule implementation begins (Week 1, Day 3):

- [ ] dart_code_linter repo accessible (tag/commit pinned)
- [ ] pyramid_lint repo accessible (tag/commit pinned)
- [ ] jfit analysis_options.yaml rules cross-referenced with this matrix
- [ ] Source file per rule documented in M4.0 spec
