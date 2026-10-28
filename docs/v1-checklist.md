# Falcon 1.0 checklist

Every hardening item and rule row below must be resolved before falcon 1.0; there is no post-1.0 tier. A hardening item or rule row is deleted once it ships, and a batch heading goes with its last item. The declarations in `crates/falcon_rules/src/lint/<group>.rs` and `crates/falcon_rules/src/cross_file.rs` are authoritative for shipped rule ids, groups, domains, execution kinds, and default enablement.

## Hardening

Engine fixes from the October 2026 audit plus the 1.0 workflow features. Every item blocks 1.0. Items are grouped into batches sized for one agent: each batch says what code it owns and what it waits on. Batches in the same stage can run in parallel unless a batch says otherwise. Each fix lands with a regression test built from its repro.

### Stage 6: editor and CI workflows

#### 24. Release credentials

Owns the repository's Actions secrets and the Homebrew tap. Releases fail before building until all of these exist; see `docs/installation.md` "Release publishing setup".

- [ ] **macOS signing and notarisation.** Get a Developer ID Application certificate and set `APPLE_CERTIFICATE_P12`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_TEAM_ID` and `APPLE_APP_PASSWORD`.
- [ ] **Homebrew tap.** Create the public `JacobDevelops/homebrew-tap` repository with an initial commit and set `HOMEBREW_TAP_TOKEN` to a fine-grained PAT with Contents read and write on it.

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
