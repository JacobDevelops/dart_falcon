# Falcon 1.0 checklist

Workflow items and must-have rules must be resolved before falcon 1.0; nice-to-have rules are stretch goals and post-1.0 rules are out of scope (see [Priorities](#priorities)). A workflow item or rule row is deleted once it ships. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` is authoritative for shipped rule ids, groups, domains, execution kinds, and default enablement.

## Workflow

- [ ] **Safe autofixes.** Add CLI preview/apply, editor quick fixes, fix-all, and fix-on-save.
- [ ] **Existing-debt baselines.** Let teams adopt falcon without fixing historical findings, matching findings robustly across line changes rather than by rule/path/line/message.
- [ ] **CI report formats.** Add SARIF first, then CodeClimate/GitLab, Checkstyle, and Sonar where users need them, plus HTML; falcon already supports JSON and text.
- [ ] **Git-aware PR review.** Analyse changes since a git ref with project context, produce a PR-focused report, and optionally write a GitHub step summary or comment. This is a workflow gap, not an engine gap.
- [ ] **Severity-based CI thresholds.** Add an explicit fail-on warnings/errors policy alongside baselines; max-errors and custom exit codes are not the same interface.
- [ ] **Easier install and update.** Add an npm wrapper that downloads and verifies binaries, self-update and version selection, cargo-binstall metadata, optional signing/notarisation, and Homebrew publishing.
- [ ] **Package-aware monorepo runs.** Detect Melos/pub workspaces, apply per-package config, and aggregate per-package reports; falcon already analyses multiple paths.

## Priorities

- **must-have**: blocks 1.0. The rule is either an official preset member or a
  high-signal correctness/idiom check expected of a serious Dart/Flutter linter.
- **nice-to-have**: 1.0 stretch. Shipped if the workstream reaches it, but the
  release does not block on it.
- **post-1.0**: explicitly out of 1.0 scope (experimental, hyper-niche, or
  needs config/schema design that is itself post-1.0 work).

**Falcon groups** are `a11y | complexity | correctness | performance | security |
style | suspicious`.

**Analysis types** (what a rule needs from the engine):

- **syntax-only**: syntax-tree pass, no types.
- **needs-local-type-inference**: local flow/types.
- **needs-full-type-resolution**: element model / SDK type hierarchies.
- **needs-cross-file/project**: whole-program or pubspec/asset analysis.

## Rule totals

| Source | must-have | nice-to-have | post-1.0 | total |
|---|---:|---:|---:|---:|
| DCM-inspired | 0 | 0 | 24 | 24 |
| Cross-file additions | 0 | 4 | 4 | 8 |
| Additional rules | 0 | 0 | 3 | 3 |
| **Total** | **0** | **4** | **31** | **35** |

Counts reflect remaining checklist rows. DCM nice-to-have rows shipped and were removed.

Additional rules are planned rows from outside the DCM catalog; their paywall column reads `n/a`. Requirements folded into existing rows are not counted again.

## Remaining DCM-inspired rules (24)

These post-1.0 candidates are from the [DCM catalog](https://dcm.dev/docs/rules/). Paywall labels reflect the original survey, not a current pricing check; `paid?` means the per-rule badge was not verified.

Analysis legend: **S** syntax-only, **LT** needs-local-type-inference, **FT**
needs-full-type-resolution, **X** needs-cross-file/project.

### 2a. core: correctness

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-not-encodable-in-to-json` | correctness | FT | paid | `toJson` returns a non-encodable value. | post-1.0 |
| `require-atomic-async-updates` | correctness | FT | paid | Read-modify-write across an await race. | post-1.0 |
| `pass-correct-accepted-type` | correctness | FT | paid | Argument type mismatch (DCM-style). | post-1.0 |
| `handle-throwing-invocations` | correctness | FT | paid | Call to throwing API without try/catch. | post-1.0 |

### 2b. core: suspicious

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-importing-entrypoint-exports` | suspicious | X | paid | Importing from an app entrypoint. | post-1.0 |
| `avoid-similar-names` | suspicious | S | paid | Confusingly similar identifiers in scope. | post-1.0 |

### 2c. core: style

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `prefer-single-declaration-per-file` | style | S | paid | One public type per file. | post-1.0 |
| `prefer-single-widget-per-file` | style | S | paid | One widget per file. | post-1.0 |
| `prefer-prefixed-global-constants` | style | S | paid | Global const naming convention. | post-1.0 |
| `match-class-name-pattern` | style | S | paid | Class names must match a configured regex. | post-1.0 |

### 2d. core: complexity

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-local-functions` | complexity | S | paid | Overuse of local functions. | post-1.0 |
| `avoid-excessive-expressions` | complexity | S | paid | Expression-length metric. | post-1.0 |

### 2e. core: performance

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `prefer-bytes-builder` | performance | FT | paid | Building byte lists without BytesBuilder. | post-1.0 |

### 2f. flutter domain

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `prefer-text-rich` | style | LT | paid | `Text.rich`/RichText guidance. | post-1.0 |
| `proper-super-calls` | correctness | FT | paid | initState/dispose super-call ordering (verify vs proper-super-init-state). | post-1.0 |
| `avoid-non-ascii-symbols` | style | S | n/a | Opt-in warning for non-ASCII symbols. | post-1.0 |

### 2g. test domain

Activated for `*_test.dart`.

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `allow-top-level-test-helpers` | suspicious | S | n/a | Opt-in convention check for top-level helpers in test files. | post-1.0 |
| `avoid-missing-test-files` | correctness | X | paid | Source file without a matching test. | post-1.0 |
| `avoid-misused-test-matchers` | correctness | FT | paid | Matcher applied to the wrong type. | post-1.0 |
| `format-test-name` | style | S | paid | Test description formatting. | post-1.0 |
| `prefer-test-structure` | style | S | paid | Arrange/act/assert structure. | post-1.0 |

### 2h. bloc domain

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-empty-build-when` | correctness | S | paid | Empty `buildWhen` callback. | post-1.0 |
| `avoid-cubits` | style | LT | paid | Opinionated: prefer Bloc over Cubit. | post-1.0 |
| `avoid-returning-value-from-cubit-methods` | style | LT | paid | Cubit methods should be void. | post-1.0 |

### 2i. riverpod domain

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `prefer-async-value-when` | style | LT | n/a | Optional style preference for AsyncValue.when. | post-1.0 |
| `avoid-notifier-constructors` | style | LT | paid | Notifiers shouldn't declare constructors. | post-1.0 |
| `prefer-single-notifier-per-file` | style | S | paid | One notifier per file. | post-1.0 |

### Deduplication notes

These eight DCM names are covered by shipped rules and are not counted in the DCM totals:

| DCM rule | listed instead as |
|---|---|
| `avoid-collection-methods-with-unrelated-types` | `collection_methods_unrelated_type` |
| `prefer-contains` | `prefer_contains` |
| `avoid-self-assignment` | `no_self_assignments` |
| `avoid-unnecessary-parentheses` | `unnecessary_parenthesis` |
| `avoid-unnecessary-statements` | `unnecessary_statements` |
| `avoid-banned-imports` | `banned-imports` |
| `avoid-self-compare` | `no-self-comparisons` |
| `prefer-boolean-prefixes` | `boolean-prefixes` |

## Planned cross-file rules (8)

Whole-project / app-wide rules live in the `cross-file` config section.
The current shipped set is defined by the cross-file entries in `RULE_METADATA`;
the rows below record the planned additions. Difficulty reflects existing
infrastructure: **A** reuses the resolved directive graph / lexical name index;
**B** needs a new pubspec index
(`dependencies`/`dev_dependencies`/`flutter.assets`); **C** needs a new
asset/l10n index (filesystem walk and/or `.arb` parsing).

**Shipped rules:** `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, `sort-pub-dependencies`, `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles`, and `banned-imports`. The four shipped official lints are not counted in the 8 planned additions below.

### 3b. Tier 2: nice-to-have (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-assets` | correctness | needs-cross-file/project | C | Asset declared in pubspec `flutter: assets:` but no string literal references its path. |
| `unused-l10n` | correctness | needs-cross-file/project | C | Generated localization key (`.arb`) never referenced through the generated accessor. |
| `architecture-layer-boundaries` | correctness | needs-cross-file/project | A | Warn when imports cross clean-architecture or feature-first layers classified heuristically by path. |
| `unexported-public-api` | style | needs-cross-file/project | A | Public declaration used across the package but never surfaced through a barrel/public library. |

### 3c. Tier 3: post-1.0 (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `missing-assets` | suspicious | needs-cross-file/project | C | String literal that looks like an asset path (`assets/…`) not declared in pubspec. |
| `unused-dev-dependencies` | correctness | needs-cross-file/project | B | Dev-dependency imported nowhere under `test/`, `tool/`, `integration_test/`. |
| `no-deep-package-imports` | correctness | needs-cross-file/project | A | Importing another package's internal file past its public barrel. |
| `duplicate-barrel-exports` | suspicious | needs-cross-file/project | A | Same symbol re-exported by two barrels → ambiguous import. |
