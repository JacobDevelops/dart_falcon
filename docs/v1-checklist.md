# Falcon 1.0 checklist

Every hardening item and rule row below must be resolved before falcon 1.0; there is no post-1.0 tier. A hardening item or rule row is deleted once it ships, and a batch heading goes with its last item. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` is authoritative for shipped rule ids, groups, domains, execution kinds, and default enablement.

## Hardening

Engine fixes from the October 2026 audit plus the 1.0 workflow features. Every item blocks 1.0. Items are grouped into batches sized for one agent: each batch says what code it owns and what it waits on. Batches in the same stage can run in parallel unless a batch says otherwise. Each fix lands with a regression test built from its repro.

### Stage 1: start now

These batches touch disjoint code. The first four fix crashes or wrong output reachable from ordinary source files.

#### 1. Parser robustness

Owns `crates/falcon_dart_parser` and its tests.

- [ ] **Exponential map/set literal parse.** `map_literal_lookahead` (`parser/expr.rs:1069`) parses the first element, rolls back, then parses it again, so each nested `{` doubles the work. Measured on the release binary: 0.19 s at depth 20, 12.4 s at depth 26. Parse the element once and wrap it as a `MapEntry` when `:` follows.
- [ ] **Exponential lambda fallback.** A failed lambda speculation (`expr.rs:810-823`) rejects on any error in the body and reparses the whole body. Decide lambda vs paren from the parameter list alone.
- [ ] **Unguarded pattern and type recursion.** `pattern.rs` and `types.rs` never call `guarded`, so deeply nested patterns or type arguments overflow the stack and kill the LSP.
- [ ] **Switch-expression arms can loop forever.** `parse_switch_expr_arms` (`expr.rs:96`) has no progress check at the depth limit. Break when an iteration consumes nothing, and audit the other `while !close` loops.
- [ ] **Quadratic top-level annotation lookahead.** `peek_kind_after_annotations` rescans to EOF on unclosed `(` every iteration (`mod.rs:803-913`), and abandoned top-level markers leave orphan nodes under `Program`. Parse annotations once, then dispatch.
- [ ] **Unbounded tree depth from left-associative chains.** Binary, postfix and `as`/`is` chains are built in loops, so parse depth is bounded but tree depth isn't, and the recursive default `Visitor` can overflow. Long chains are valid Dart (generated string concatenation), so don't count them toward the depth limit. Walk chains with an explicit stack in the default `Visitor` so any length is safe.
- [ ] **Adversarial parser corpus.** `corpus_tests.rs` hard-codes `/home/jacob/...` and silently skips elsewhere. Check in the crash witnesses above, walk each tree and call the accessors; consolidate the cloned `parser_tests.rs` cases behind a macro.

#### 2. Constant evaluator

Owns `crates/falcon_analyze/src/constant_value.rs` evaluation internals.

- [ ] **Constant evaluation has no size budget.** `const aN = aN-1 + aN-1;` chains grow exponentially: 560 MB RSS at N=24. Add a byte/node budget to `ConstEvaluator` and stop deep-cloning cached values.
- [ ] **Constant reference chains recurse.** Each reference recurses through `resolve_name → initializer` and caches only after returning (`constant_value.rs:971-979`), so a long acyclic chain (`a1 = a0; a2 = a1; …`) overflows the stack: a cold evaluation of the last link resolves 20,000 links in 18 ms but aborts by 100,000 on the 256 MB rules stack. The chain also counts against the 32-deep constructor limit (`constant_value.rs:1378`). Cycles already stop through the `Evaluating` marker. Evaluate a constant's dependencies first with an explicit worklist, caching each one, so chains of any length resolve without recursion or a depth limit, and the constructor limit only counts constructors.
- [ ] **Quadratic time on constant chains outside the evaluator.** `falcon check` on the same chains takes 0.6 s at 1,000 links, 2.3 s at 2,000, 9.7 s at 4,000 and 48 s at 8,000, while the evaluator alone is linear. Profile which pass re-walks the chain per declaration and make it linear.

#### 3. `unnecessary-nullable` and cross-file resolution

Owns `crates/falcon_rules/src/cross_file/unnecessary_nullable.rs`, `RuleKind::CrossFile`, and the resolution decision in `falcon_cli/src/analyze_pipeline.rs` and `falcon_lsp/src/state.rs`.

- [ ] **`unnecessary-nullable` false positives.** It misses tear-offs (`this._m`, cascade fields) and any non-literal-null write to the parameter (`cross_file/unnecessary_nullable.rs:560-600`), so it tells users to remove a `?` the code needs.
- [ ] **Cross-file rules can't request resolution.** `RuleKind::CrossFile` has no resolution flag, so `unnecessary-nullable` builds its own SDK-less indexes whenever no file rule needs resolution and its output depends on unrelated rules. Add the flag, compute resolution over both registries, delete the fallback builder.

#### 4. Identity and analysis context

Owns `crates/falcon_analyze/src/resolve/identity.rs`, `context.rs`, and the 13 rule sites matching `DeclarationIdentity::Project`.

- [ ] **`DeclarationIdentity::Project` covers external names.** SDK/package names that collide across libraries get `Project` (`identity.rs:1051-1091`), and 13 rule sites read it as "user code". Rename to `Library` and add `IdentityIndex::is_project`.
- [ ] **Intern project identity names.** `Project { name: String }` allocates on every clone, while SDK and package identities already use `InternedString`.
- [ ] **Sibling context built with the owner's path.** `use_late_for_private_fields_and_variables.rs:102-110` pairs a sibling tree with the owner's `file_path`, so path-keyed facts come from the wrong file. Add `AnalyzeContext::for_file` and make the fields private.

#### 5. Tooling and CI

Owns `xtask/`, `.github/`, `Makefile.toml`, `.cargo/config.toml`, `extensions/` and `website/`.

- [ ] **Docgen and website in CI.** Add `docgen --check`, a website typecheck/build job, and stop casting `rules.json` through `unknown`.
- [ ] **Docgen fails on gaps.** Empty descriptions and zero-diagnostic fixtures only print; the temp config path is fixed and a write failure falls back silently.
- [ ] **`validate-rules --rule <typo>` fails.** A filter matching nothing currently reports PASS.
- [ ] **Honour `CARGO_TARGET_DIR`.** `validate-rules`, docgen and `makers validate_rules` hard-code `target/debug/falcon`.
- [ ] **Release token exposure.** `prepare-release.yml` persists `RELEASE_PAT` in `.git/config` while dependency build scripts compile; the `cargo xtask` alias lacks `--locked`.
- [ ] **Workflow hygiene.** Top-level `permissions: contents: read`, PR `concurrency` cancel, one version parser.
- [ ] **Release-pinned schema URL.** `$schema` and the VS Code validation point at `main`, so released binaries validate against unreleased rule sets.
- [ ] **Extensions in the gates.** `falcon-zed` is its own edition-2021 workspace outside fmt/clippy/build; the VS Code extension has no lockfile and both sit outside `xtask version`.
- [ ] **Split xtask.** One 2,136-line `main.rs` doing six jobs, with `perf-lock` undocumented.

#### 6. Docs and changesets

Owns `.changeset/`, `README.md`, `ROADMAP.md`, `docs/` and the count assertions in `meta_tests.rs`.

- [ ] **Readable changesets.** 137 of 178 per-rule changesets never name their rule, and patch changesets fix rules that were never released. Name each rule id, fold fixes into the add entry, and add a `meta_tests.rs` check that a changeset named after a rule mentions its id.
- [ ] **Derived rule counts.** README, getting-started, installation and ROADMAP hard-code 513/215, and `recommended_rules_match_documented_count` never reads them.
- [ ] **Stale docs.** ROADMAP's changelog plan and the undocumented `include_assertions` option.

### Stage 2: after a stage 1 batch

Each batch here edits files a stage 1 batch owns, so it starts once that batch lands.

#### 7. Syntax model

Owns `crates/falcon_syntax`, `crates/falcon_dart_parser`, and the feature-gate check in `falcon_analyze/src/environment/external.rs`. After batch 1.

- [ ] **Generated syntax kind mapping.** `Expr::cast` ends in `_ => None` and the visitor schema test asserts hand counts. Generate `cast`/`syntax` from `expression_enum!` and test that the default walk reaches every node.
- [ ] **One parameter-kind encoding.** Parameter kinds are encoded both as marker nodes and as tree-builder flags (`tree.rs:98-414`).
- [ ] **Typed feature-gate parse errors.** `external.rs:1276` decides external-library resolution by matching the message prefix `"This requires the '"`. Add a `ParseErrorKind`.
- [ ] **Lexer and parser leftovers.** Unclosed `${` swallows the rest of the file; the nested-comment scan in `language_version.rs` is quadratic; the `in_ctor_init_value` latch isn't restored; the cascade selector loop has drifted from `parse_postfix`; `parse_field_or_method` keeps errors from the abandoned attempt.

#### 8. CLI and config behaviour

Owns `crates/falcon_cli`, `crates/falcon_config`, `crates/falcon_diagnostics` output, and path matching in `falcon_lsp`. After batch 3, which owns the resolution decision in `analyze_pipeline.rs` and `state.rs`.

- [ ] **Relative globs don't match walked paths.** `falcon check .` yields `./build/…`, so the documented `"!build/**"` excludes nothing (reproduced). The LSP matches absolute paths, so CLI and editor select different files. Resolve every glob relative to the config file's directory before matching files, overrides and diagnostics. `*` stops matching at `/` and `**` crosses directories; that changes how some existing configs match, so it ships with a changeset and a migrate note.
- [ ] **Exit code holes.** A missing path is skipped, no paths lints nothing, and `--max-errors 0` exits 0 because the code is computed after truncation (`analyze_pipeline.rs:142-148,243-282`). No paths lints the current directory. A missing path, or a run that processes no files, is an error diagnostic with a non-zero exit, with a `--no-errors-on-unmatched` flag for tools like lint-staged that may pass nothing. Pipeline failures exit 1 like lint errors. `--max-errors` only limits what's printed, so the exit code comes from every diagnostic.
- [ ] **Silent config typos.** Nested unknown keys are ignored, an override with a mistyped `includes` applies to every file, and domain names aren't validated (`falcon_config/src/lib.rs:47-85,167,353,664-732`). An unknown key at any depth, or an unknown domain, is an error naming the full key path, and the config doesn't load. Legacy spellings stay accepted with a deprecation warning. A mistyped `includes` then fails to load, so overrides without `includes` keep applying everywhere.
- [ ] **Library code returns warnings.** `falcon_config` and `falcon_rules` `eprintln!` on every registry build, so the LSP never shows them. Return warnings, and provide one loader that canonicalises.
- [ ] **One canonicalisation.** `meta::canonicalize_rules` and `migrate::canonicalize_group` rank and merge differently; every loader must remember to call it.
- [ ] **Migrate coverage.** `run_migrate` has no tests and an in-place upgrade reorders keys (`serde_json` without `preserve_order`).
- [ ] **JSON output contract.** Severity serialises as `"Error"` while text prints lowercase, and `0` means unresolved line/col. Emit lowercase severities and omit unresolved positions, then snapshot a non-empty output.
- [ ] **Single pruning walk.** Excluded directories are fully traversed and manifests are found by a second walk.

#### 9. Fix model

Owns `crates/falcon_diagnostics` and `AnalyzeContext`'s diagnostic API. After batch 4, which owns `context.rs`, and batch 8, which changes how `Diagnostic` serialises. Design only; rules adopt it in batch 10.

- [ ] **Fix model.** `Diagnostic` has no fix field today. Add a fix carrying text edits over the lossless CST and a kind, safe or unsafe, declared by the rule. A safe fix preserves the program's behaviour; a fix that changes behaviour is unsafe even when the diagnostic is correct. Add the rule-facing API to attach one, so the diagnostic API changes once together with batch 10.

### Stage 3: rule API

These change the API every rule uses, so they run one at a time and no stage 4 batch starts until batch 10 lands.

#### 10. Rule declaration and ids

Owns `crates/falcon_rules/src/meta.rs`, the `lint/<group>.rs` files, `registry.rs`, every rule's `ctx.diagnostic` call, and codegen in xtask. After batch 9.

- [ ] **Single rule declaration.** Adding a rule touches about 15 places, more than half unchecked. Replace the hand-written `RULE_METADATA` entries and `pub mod` lines with a per-group declaration macro that emits the module, metadata and `const NAME`, and split `meta.rs` into per-group tables.
- [ ] **Diagnostic rule ids aren't enforced.** Rules retype their id in every `ctx.diagnostic("…")`, and the corpus test drops foreign ids (`corpus_tests.rs:489`), so a typo gives a warning nobody can configure or suppress. Have the registry stamp the id per slot.
- [ ] **Derive `needs_resolution`.** The `file`/`resolved` choice is hand-set and independent of what `prepare` returns. Assert `Semantic` preparation implies resolution in `meta_tests`.
- [ ] **Registry invariants.** Assert alias keys don't shadow live rule names; drop the unused `domain` parameter on cross-file rules; replace the linear `meta_for` scan with a map.
- [ ] **Codegen matches the procedure.** Compile-test the stub template, list schema/docgen/changeset steps, scaffold cross-file rules, validate the kebab-case id, delete the dead group-exists branch.

#### 11. Typed rule options

Owns `lint/options.rs`, the 46 option-reading rules, `schema.rs` and the options table in `docs/configuration.md`. After batch 10.

Option keys are kebab-case like every other config key. The current snake_case keys keep loading with a deprecation warning, and `falcon migrate` rewrites them.

- [ ] **Typed rule options.** Options are `serde_json::Value` read by string key in 46 files, and bad values silently disable behaviour (`prefer_correct_handler_name` drops an invalid regex). Give each rule a `Deserialize + JsonSchema` options struct with `deny_unknown_fields`, emit per-rule schemas, and generate the `configuration.md` options table.

### Stage 4: rule internals

After batch 10. Batches 12 and 13 can run in parallel; 14 runs after 12 because both move code out of `use_build_context_synchronously`.

#### 12. Shared rule helpers

Owns rule files' helper exports, a new `lint` support module, and `falcon_syntax` helper methods.

- [ ] **Rule files aren't libraries.** About 25 rule files export helpers to other rules (`use_build_context_synchronously`, `always_specify_types`, `conditional_uri_does_not_exist`, …). Move helpers to a support module, keep group directories rule-only, drop the `#[path]` include, and add a ratchet against rule-to-rule imports.
- [ ] **One resolved predicate per concept.** 8 `@override` detectors with 3 meanings, 6 `@deprecated` detectors (half name-only), drifted test-package lists, 13 copies of span containment. Add `has_override`, `deprecation`, `Span::contains`/`contains_offset` and route the test-package checks through `test_assertions`.
- [ ] **Syntax helpers instead of rule copies.** `BinaryOp`/`AssignOp::operator_name`, `TopLevelDecl::type_params`, and arity from the indexes instead of the table in `always_specify_types.rs:116-140`.
- [ ] **Resolved names instead of spelling.** `avoid_returning_widgets`, `member_ordering`, `avoid_returning_null_for_void` and `avoid_catches_without_on_clauses` match `Widget`/`State`/`Future` by name.
- [ ] **Small rule cleanups.** Remove the parallel declarations iterator and its `expect` in `prefer_const_constructors_in_immutables`; replace guarded `unwrap`s with slice patterns; add a helper for the ~60 identical `prepare` bodies; interval lookup in `deprecated_member_use_from_same_package`.

#### 13. Rule tests and docs

Owns rule `#[cfg(test)]` modules, `crates/falcon_rules/tests/`, rule messages and `//!` docs.

- [ ] **Rule test support.** 19 files hand-build identity harnesses and 12 build SDK trees. Add a shared `#[cfg(test)]` support module, move misplaced tests to the rule they test, and move `lint/semantic_scope_tests.rs` to `falcon_analyze`.
- [ ] **Corpus checks messages in-process.** `cargo test` ignores `msg:` and only reads top-level fixtures; only the xtask harness compares messages. Check them in both and fix the CONTRIBUTING wording.
- [ ] **Message style.** 61 messages lack terminal punctuation and code is quoted three ways. Messages are full sentences ending in a full stop, with code in backticks; enforce it in a meta test.
- [ ] **User-facing module docs.** Several `//!` docs read as fix logs and docgen publishes them; `unnecessary_nullable`'s doc is stale and `prefer_iterable_every`'s describes the wrong semantics.

#### 14. Shared analysis for rules

Owns `AnalyzeContext` caches, `semantic_scope.rs` queries, `const_eval.rs`, `ConstantDeclaration` in `constant_value.rs`, the derives on `DeclarationIdentity`, `cross_file.rs` package helpers, and the rules migrated onto them. After batch 12; batches 2 and 4 have already landed by then.

- [ ] **Cache shared per-file facts.** `FunctionBodies`, const contexts and `local_declarations` are re-walked by 16-20 rules per file. Cache them on `AnalyzeContext`.
- [ ] **One constant-context definition.** `const_eval.rs` has two `ConstContexts` that disagree; one is a linear scan per expression.
- [ ] **Delete `ConstantDeclaration`.** It mirrors `DeclarationIdentity` with five hand-written conversions; derive `Ord` on `DeclarationIdentity` instead. `const_eval` should reuse the evaluator's custom-`==` check and constant walk.
- [ ] **Package identity from the analysis environment.** Per-file rules do filesystem IO through the global `PACKAGE_CACHE` (`cross_file.rs:379-446`), and package discovery exists three times. Resolve it once per file into the context.
- [ ] **One async-gap engine.** `check_is_not_closed_after_async_gap` reimplements a straight-line model next to the `use_build_context_synchronously` engine. Parameterise the shared engine by guard.
- [ ] **Shared reference resolution.** `unreachable_from_main`, `deprecated_member_use_from_same_package`, `use_late_for_private_fields_and_variables` and `unnecessary_nullable` each carry their own engine. Add `SemanticState::referenced_declaration` and migrate them; `unreachable_from_main` also moves to the shared doc-reference parser.

### Stage 5: analysis core

Can run alongside stages 3 and 4 except where a batch touches rule files. Batch 15 goes first; 16 and 17 then work in separate modules.

#### 15. Split `resolve/semantic.rs`

Owns `crates/falcon_analyze/src/resolve/semantic.rs` and its new submodules. Pure moves.

- [ ] **Split `resolve/semantic.rs`.** 22k lines, 37% inline tests. Move tests out first, then split along the existing seams (types, substitution, model, index build/fixpoint/SDK/hierarchy/members, inference env/call/expr/function/collections, join).

#### 16. Type model

Owns the semantic index, `type_index.rs`, `ResolvedType` and the rule sites that construct or match it. After batch 15.

- [ ] **One `TypeFacts` record.** About 12 parallel per-type maps are kept in sync by hand and coupled by index arithmetic for enum/mixin supertypes (`semantic.rs:3962-4005,5815-5830`).
- [ ] **Macro-generated fallback merge.** `add_fallback_signatures` copies 10 named tables by hand even though the `signature_index!` macro claims one-place wiring.
- [ ] **One type hierarchy and SDK table.** `TypeIndex` and `SignatureIndex` each carry subtype logic and curated SDK tables.
- [ ] **One relation API.** Seven overlapping subtype queries and three `unrelated` variants; `is_subtype_of` returns `Truth` but never `No`.
- [ ] **Hide `ResolvedType` internals.** 7 rule sites build `Interface { occurrence: Default::default(), … }` and 328 match its variants. Add constructors and views, make metadata fields crate-private.
- [ ] **Liveness-safe memo keys.** Memo caches key on raw pointers guarded only by comments; wrap them in a borrowed-key newtype or intern types.

#### 17. Scoping and flow

Owns `lexical_usage.rs`, `flow.rs`, `flow_writes.rs`, `semantic_scope.rs` flow handling, the inference environment module, and the element trait in `falcon_syntax`. After batch 15.

- [ ] **One lexical-scope resolver.** `lexical_usage`, `flow_writes`, `TypeEnvironment` and `semantic_scope` each reimplement Dart scoping by string.
- [ ] **Shared flow joins.** `ScopedWalker` and `ClosureReturns` both hand-roll if/switch/try/loop joins. Move them into `FlowContext`, and move `switch_is_exhaustive` back to `flow.rs`.
- [ ] **Shared collection/map element visiting.** Eleven visitors duplicate `visit_collection_element`/`visit_map_element`. Add an element trait in `falcon_syntax`.
- [ ] **Terminating fixpoint.** `converge`/`infer_programs` (`semantic.rs:4641-4695`) rescan every program, SDK included, each round, and only end if every pass makes monotone progress, which nothing enforces. Add a worklist that requeues only declarations whose inputs changed. Enforce at the type-map writes that a step only fills an unknown type or tightens a partial one, never replaces a concrete type. Keep a round cap as a backstop; hitting it is an internal error naming the declarations still changing, not silently half-inferred types.

#### 18. Analyze infrastructure

Owns `parallel.rs`, `environment/`, `registry.rs` threading and `lib.rs` exports. Any time after batch 15.

- [ ] **Log swallowed IO errors.** Package config, lockfile, `libraries.json`, SDK version and external sources fail silently with `.ok()`.
- [ ] **One stack-sized thread pool.** Each parse and rules pass spawns its own 256 MB-stack thread, with retry machinery for spawn failures and inconsistent panic handling.
- [ ] **Narrow analyze entry points.** 11 `evaluate_*` variants, 6 `parallel` entry points (4 test-only), triplicated oversize filtering, every module `pub` plus ~80 re-exports.
- [ ] **Analysis cleanups.** Typed receiver slot instead of `"this"`; `dyn` lookup closures; Fx hashing for substitution maps; per-alias cache without a mutex; stable keys in `group_instances`; bounded external caches; suppressions reuse `LineIndex`.

### Stage 6: editor and CI workflows

#### 19. Shared pipeline and incremental LSP

Owns `crates/falcon_lsp` and the shared pipeline extracted from `falcon_cli`. After batches 3 and 8.

- [ ] **One project pipeline.** The CLI and LSP load config, walk files, build `ProjectFile`s, build indexes and sort diagnostics differently. Share one pipeline and test that both produce the same diagnostics.
- [ ] **Incremental LSP.** Every edit pause re-walks and re-parses the workspace twice on one thread, with `$/cancelRequest` ignored. Cache disk parses, invalidate via watched files, rebuild only what changed.
- [ ] **LSP roots from `initialize`.** The workspace root comes from the process cwd; config errors never reach the editor.
- [ ] **LSP position mapping.** `format_lsp` rescans the file per diagnostic; use one `LineIndex` per publish.
- [ ] **No cross-file diagnostic flicker.** Per-file publish then merged republish makes cross-file squiggles blink on every edit.
- [ ] **LSP test placement.** About 1.8k lines of `state.rs` and 770 of `server.rs` are inline tests; move them to child test modules.

#### 20. Autofix delivery

Owns the CLI fix commands and LSP code actions. After batches 9 and 19.

- [ ] **Safe autofixes.** `--write` applies safe fixes and `--write --unsafe` also applies unsafe ones, with a preview mode. Fixes apply one at a time with re-analysis in between until nothing changes, so overlapping edits never merge. If a file's text repeats, fixes are undoing each other: stop and report the rules involved. An iteration cap stays as a backstop for fixes that grow the file without repeating, and hitting it is reported, not silent. A per-rule `fix: "none" | "safe" | "unsafe"` option, also allowed in overrides, turns a fix off or reclassifies it. Editors offer both kinds as quick fixes plus fix-all; fix-on-save applies safe fixes only.

#### 21. CI adoption

Owns CLI reporting and exit policy. After batch 8.

- [ ] **Existing-debt baselines.** Let teams adopt falcon without fixing historical findings, matching findings robustly across line changes rather than by rule/path/line/message.
- [ ] **Severity-based CI thresholds.** Today any diagnostic, warnings included, fails the run (`analyze_pipeline.rs:247-251`). Make warnings non-failing by default and add `--error-on-warnings` plus a config equivalent; this changes CI results for existing users, so it ships with a changeset.
- [ ] **CI report formats.** Add SARIF first, then CodeClimate/GitLab, Checkstyle, and Sonar where users need them, plus HTML; falcon already supports JSON and text.

#### 22. Git-aware PR review

After batch 21.

- [ ] **Git-aware PR review.** Analyse changes since a git ref with project context, produce a PR-focused report, and optionally write a GitHub step summary or comment. This is a workflow gap, not an engine gap.

#### 23. Package-aware monorepo runs

After batch 19.

- [ ] **Package-aware monorepo runs.** Detect Melos/pub workspaces, apply per-package config, and aggregate per-package reports; falcon already analyses multiple paths.

#### 24. Install and update

Owns release packaging. Independent of the engine; can start any time.

- [ ] **Easier install and update.** Add self-update and version selection to the `falcon` launcher, cargo-binstall metadata, optional signing/notarisation, and Homebrew publishing.

### Stage 7: last

#### 25. Clippy pedantic

Touches every crate, so it runs alone after the refactors above; earlier it would conflict with every parallel batch, and its `too_many_lines` hits overlap batches 15-17.

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
