# Falcon 1.0 checklist

Every hardening item and rule row below must be resolved before falcon 1.0; there is no post-1.0 tier. A hardening item or rule row is deleted once it ships, and a batch heading goes with its last item. The declarations in `crates/falcon_rules/src/lint/<group>.rs` and `crates/falcon_rules/src/cross_file.rs` are authoritative for shipped rule ids, groups, domains, execution kinds, and default enablement.

## Hardening

Engine fixes from the October 2026 audit plus the 1.0 workflow features. Every item blocks 1.0. Items are grouped into batches sized for one agent: each batch says what code it owns and what it waits on. Batches in the same stage can run in parallel unless a batch says otherwise. Each fix lands with a regression test built from its repro.

### Stage 6: editor and CI workflows

#### 19. Shared pipeline and incremental LSP

Owns `crates/falcon_lsp` and the shared pipeline extracted from `falcon_cli`. Can start any time.

- [ ] **One project pipeline.** The CLI and LSP load config, walk files, build `ProjectFile`s, build indexes and sort diagnostics differently. Share one pipeline and test that both produce the same diagnostics.
- [ ] **Incremental LSP.** Every edit pause re-walks and re-parses the workspace twice on one thread, with `$/cancelRequest` ignored. Cache disk parses, invalidate via watched files, rebuild only what changed.
- [ ] **LSP roots from `initialize`.** The workspace root comes from the process cwd; config errors never reach the editor.
- [ ] **LSP position mapping.** `format_lsp` rescans the file per diagnostic; use one `LineIndex` per publish.
- [ ] **No cross-file diagnostic flicker.** Per-file publish then merged republish makes cross-file squiggles blink on every edit.
- [ ] **LSP test placement.** About 1.8k lines of `state.rs` and 770 of `server.rs` are inline tests; move them to child test modules.

#### 20. Autofix delivery

Owns the CLI fix commands and LSP code actions. After batch 19.

- [ ] **Safe autofixes.** `--write` applies safe fixes and `--write --unsafe` also applies unsafe ones, with a preview mode. Fixes apply one at a time with re-analysis in between until nothing changes, so overlapping edits never merge. If a file's text repeats, fixes are undoing each other: stop and report the rules involved. An iteration cap stays as a backstop for fixes that grow the file without repeating, and hitting it is reported, not silent. A per-rule `fix: "none" | "safe" | "unsafe"` option, also allowed in overrides, turns a fix off or reclassifies it. Editors offer both kinds as quick fixes plus fix-all; fix-on-save applies safe fixes only.

#### 21. CI adoption

Owns CLI reporting and exit policy. Can start any time.

- [ ] **Existing-debt baselines.** Let teams adopt falcon without fixing historical findings, matching findings robustly across line changes rather than by rule/path/line/message.
- [ ] **Severity-based CI thresholds.** Today any diagnostic, warnings included, fails the run (`analyze_pipeline.rs:247-251`). Make warnings non-failing by default and add `--error-on-warnings` plus a config equivalent; this changes CI results for existing users, so it ships with a changeset.
- [ ] **CI report formats.** Add SARIF first, then CodeClimate/GitLab, Checkstyle, and Sonar where users need them, plus HTML; falcon already supports JSON and text.

#### 23. Package-aware monorepo runs

After batch 19.

- [ ] **Package-aware monorepo runs.** Detect Melos/pub workspaces, apply per-package config, and aggregate per-package reports; falcon already analyses multiple paths.

#### 24. Install and update

Owns release packaging. Independent of the engine; can start any time.

- [ ] **Easier install and update.** Add self-update and version selection to the `falcon` launcher, cargo-binstall metadata, optional signing/notarisation, and Homebrew publishing.

#### 25. End-to-end OSS corpus

Owns a new `cargo xtask e2e` and its CI jobs. Independent of the engine; can start any time.

- [ ] **e2e OSS corpus.** Run falcon over pinned, shallow clones of large Dart/Flutter repos (immich, AppFlowy, flutter/packages, flame, LocalSend, bloc, riverpod, Wonderous, flutter/samples, dart-lang/sdk `tests/language`). Catch panics and parser/internal errors, snapshot diagnostic counts, and smoke-test migration of common `analysis_options.yaml` presets. A fast subset runs in PR CI and the full set nightly.

#### 27. Remaining recursive walkers

Owns the rule and analysis helpers listed below. Can start any time.

- [ ] **Long chains in the remaining walkers.** `falcon check` survives 100,000-operand `+`, `&&`, `||`, `??` and string chains as initializers, arguments and statements, but these walkers still recurse once per operand. Default on: parameter defaults (`default_syntax.rs` `SyntaxCollector`), `==`/`!=` operands in `async_semantics.rs` `BoundNames`, the async-gap fallback in `async_state.rs` `Tree`, switch guards in `avoid_duplicate_switch_case_conditions.rs` `GuardProof`, `kDebugMode` branches in `avoid_print`, `Target` annotation arguments in `library_annotations`, generator bodies in `null_check_on_nullable_type_parameter`, sibling parts in `prefer_final_fields`, and dispose bodies in `controller_disposal.rs`. Default off: `support/constants.rs`, `const_eval.rs`, `logical_operands.rs`, `unnecessary_nullable`, `avoid_complex_arithmetic_expressions`, `move_variable_outside_iteration`, `cascade_invocations`, `prefer_moving_to_variable`, `deprecated_member_use_from_same_package`, `prefer_foreach`, `avoid_catches_without_on_clauses`, `avoid_redundant_argument_values`, `avoid_types_on_closure_parameters`, `prefer_int_literals`, `unreachable_from_main` and `use_late_for_private_fields_and_variables`. Route each through `walk_expr_chain`, and extend `tests/long_expression_chains.rs` with one 100,000-operand case per shape, run with the rule enabled.

### Stage 7: last

#### 26. Clippy pedantic

Touches every crate, so it runs alone after the refactors above; earlier it would conflict with every parallel batch.

- [ ] **Clippy pedantic.** Enable `pedantic` in `[workspace.lints.clippy]`, inherited by every crate, under the existing `-D warnings` gate. A trial run reported 4,546 warnings, 3,492 with machine-applicable suggestions.
- [ ] **Decompose long functions.** Clippy's `too_many_lines` flags 242 functions over 100 lines (`infer_non_call` 649, `semantic_scope::visit_expr` 568, `add_declarations` 532). Moving files doesn't fix these.

## Legend

**Falcon groups** are `a11y | complexity | correctness | performance | security |
style | suspicious`.

**Analysis types** (what a rule needs from the engine):

- **syntax-only**: syntax-tree pass, no types.
- **needs-local-type-inference**: local flow/types.
- **needs-full-type-resolution**: element model / SDK type hierarchies.
- **needs-cross-file/project**: whole-program or pubspec/asset analysis.

## Rule totals

| Source | remaining |
|---|---:|
| DCM-inspired | 24 |
| Cross-file additions | 8 |
| Additional rules | 3 |
| **Total** | **35** |

Counts reflect remaining checklist rows. Every DCM must-have and nice-to-have row has shipped and was removed.

Additional rules are planned rows from outside the DCM catalog; their paywall column reads `n/a`. Requirements folded into existing rows are not counted again.

## Remaining DCM-inspired rules (24)

These rules are from the [DCM catalog](https://dcm.dev/docs/rules/). Paywall labels reflect the original survey, not a current pricing check; `paid?` means the per-rule badge was not verified.

Analysis legend: **S** syntax-only, **LT** needs-local-type-inference, **FT**
needs-full-type-resolution, **X** needs-cross-file/project.

### 2a. core: correctness

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-not-encodable-in-to-json` | correctness | FT | paid | `toJson` returns a non-encodable value. |
| `require-atomic-async-updates` | correctness | FT | paid | Read-modify-write across an await race. |
| `pass-correct-accepted-type` | correctness | FT | paid | Argument type mismatch (DCM-style). |
| `handle-throwing-invocations` | correctness | FT | paid | Call to throwing API without try/catch. |

### 2b. core: suspicious

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-importing-entrypoint-exports` | suspicious | X | paid | Importing from an app entrypoint. |
| `avoid-similar-names` | suspicious | S | paid | Confusingly similar identifiers in scope. |

### 2c. core: style

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `prefer-single-declaration-per-file` | style | S | paid | One public type per file. |
| `prefer-single-widget-per-file` | style | S | paid | One widget per file. |
| `prefer-prefixed-global-constants` | style | S | paid | Global const naming convention. |
| `match-class-name-pattern` | style | S | paid | Class names must match a configured regex. |

### 2d. core: complexity

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-local-functions` | complexity | S | paid | Overuse of local functions. |
| `avoid-excessive-expressions` | complexity | S | paid | Expression-length metric. |

### 2e. core: performance

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `prefer-bytes-builder` | performance | FT | paid | Building byte lists without BytesBuilder. |

### 2f. flutter domain

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `prefer-text-rich` | style | LT | paid | `Text.rich`/RichText guidance. |
| `proper-super-calls` | correctness | FT | paid | initState/dispose super-call ordering (verify vs proper-super-init-state). |

### 2g. test domain

Activated for `*_test.dart`.

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-missing-test-files` | correctness | X | paid | Source file without a matching test. |
| `avoid-misused-test-matchers` | correctness | FT | paid | Matcher applied to the wrong type. |
| `format-test-name` | style | S | paid | Test description formatting. |
| `prefer-test-structure` | style | S | paid | Arrange/act/assert structure. |

### 2h. bloc domain

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-empty-build-when` | correctness | S | paid | Empty `buildWhen` callback. |
| `avoid-cubits` | style | LT | paid | Opinionated: prefer Bloc over Cubit. |
| `avoid-returning-value-from-cubit-methods` | style | LT | paid | Cubit methods should be void. |

### 2i. riverpod domain

| DCM rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-notifier-constructors` | style | LT | paid | Notifiers shouldn't declare constructors. |
| `prefer-single-notifier-per-file` | style | S | paid | One notifier per file. |

### Deduplication notes

These eight DCM names are covered by shipped rules and are not counted in the DCM totals:

| DCM rule | listed instead as |
|---|---|
| `avoid-collection-methods-with-unrelated-types` | `collection-methods-unrelated-type` |
| `prefer-contains` | `prefer-contains` |
| `avoid-self-assignment` | `no-self-assignments` |
| `avoid-unnecessary-parentheses` | `unnecessary-parenthesis` |
| `avoid-unnecessary-statements` | `unnecessary-statements` |
| `avoid-banned-imports` | `banned-imports` |
| `avoid-self-compare` | `no-self-comparisons` |
| `prefer-boolean-prefixes` | `boolean-prefixes` |

## Remaining additional rules (3)

These rules come from outside the DCM catalog.

| rule | falcon group | analysis | paywall | description |
|---|---|---|---|---|
| `avoid-non-ascii-symbols` | style | S | n/a | Opt-in warning for non-ASCII symbols. |
| `allow-top-level-test-helpers` | suspicious | S | n/a | Opt-in convention check for top-level helpers in test files. |
| `prefer-async-value-when` | style | LT | n/a | Optional style preference for AsyncValue.when. |

## Planned cross-file rules (8)

Whole-project / app-wide rules live in the `cross-file` config section.
The current shipped set is defined by the cross-file entries in `RULE_METADATA`;
the rows below record the planned additions. Difficulty reflects existing
infrastructure: **A** reuses the resolved directive graph / lexical name index;
**B** needs a new pubspec index
(`dependencies`/`dev_dependencies`/`flutter.assets`); **C** needs a new
asset/l10n index (filesystem walk and/or `.arb` parsing).

**Shipped rules:** `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, `sort-pub-dependencies`, `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles`, and `banned-imports`. The four shipped official lints are not counted in the 8 planned additions below.

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-assets` | correctness | needs-cross-file/project | C | Asset declared in pubspec `flutter: assets:` but no string literal references its path. |
| `unused-l10n` | correctness | needs-cross-file/project | C | Generated localization key (`.arb`) never referenced through the generated accessor. |
| `architecture-layer-boundaries` | correctness | needs-cross-file/project | A | Warn when imports cross clean-architecture or feature-first layers classified heuristically by path. |
| `unexported-public-api` | style | needs-cross-file/project | A | Public declaration used across the package but never surfaced through a barrel/public library. |
| `missing-assets` | suspicious | needs-cross-file/project | C | String literal that looks like an asset path (`assets/…`) not declared in pubspec. |
| `unused-dev-dependencies` | correctness | needs-cross-file/project | B | Dev-dependency imported nowhere under `test/`, `tool/`, `integration_test/`. |
| `no-deep-package-imports` | correctness | needs-cross-file/project | A | Importing another package's internal file past its public barrel. |
| `duplicate-barrel-exports` | suspicious | needs-cross-file/project | A | Same symbol re-exported by two barrels → ambiguous import. |
